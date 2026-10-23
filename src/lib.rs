use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, IoSlice, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    Record,
    Get,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Config {
    pub mode: Mode,
    pub log_path: PathBuf,
}

pub fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Option<Config>, String> {
    let mut mode = Mode::Record;
    let mut log_path: Option<PathBuf> = None;

    let mut args_iter = args.into_iter();
    let _prog = args_iter.next();

    for arg in args_iter {
        let arg_str = arg.to_string_lossy();
        match arg_str.as_ref() {
            "-g" | "--get" => {
                mode = Mode::Get;
            }
            "-h" | "--help" => {
                print_help();
                return Ok(None);
            }
            "-v" | "--version" => {
                println!("cdlog {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            _ => {
                if arg_str.starts_with('-') {
                    return Err(format!("Unknown option: {}", arg_str));
                }
                if log_path.is_none() {
                    log_path = Some(PathBuf::from(arg));
                }
            }
        }
    }

    let log_path = log_path.ok_or_else(|| "CDLOG_PATH argument is required".to_string())?;

    Ok(Some(Config {
        mode,
        log_path,
    }))
}

fn print_help() {
    println!(
        r#"cdlog {}
A fast cd history logger and fuzzy finder backend.

USAGE:
    cdlog [OPTIONS] <CDLOG_PATH>

ARGS:
    <CDLOG_PATH>    Path to cdlog file

OPTIONS:
    -g, --get       Output recorded paths except the path provided in stdin
    -h, --help      Print help information
    -v, --version   Print version information
"#,
        env!("CARGO_PKG_VERSION")
    );
}

struct TmpFileGuard<'a>(&'a Path);

impl<'a> Drop for TmpFileGuard<'a> {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.0);
    }
}

/// Trims trailing '\r' and '\n' from a byte slice.
#[inline]
pub fn trim_newlines(mut bytes: &[u8]) -> &[u8] {
    while let Some((&last, rest)) = bytes.split_last() {
        if last == b'\n' || last == b'\r' {
            bytes = rest;
        } else {
            break;
        }
    }
    bytes
}

/// Reads the first line from reader, trimming trailing newlines.
fn read_single_path<R: Read>(mut reader: R) -> io::Result<Vec<u8>> {
    let mut line = Vec::new();
    let mut buf = [0u8; 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        for &b in &buf[..n] {
            if b == b'\n' {
                let trimmed = trim_newlines(&line);
                return Ok(trimmed.to_vec());
            }
            line.push(b);
        }
    }
    let trimmed = trim_newlines(&line);
    Ok(trimmed.to_vec())
}

/// Searches for an exact full-line match using Boyer-Moore-Horspool algorithm.
/// Returns (start_index, end_index_including_newline) on match.
pub fn find_exact_line(content: &[u8], target: &[u8]) -> Option<(usize, usize)> {
    let m = target.len();
    let n = content.len();
    if m == 0 || n < m {
        return None;
    }

    let mut shift = [m; 256];
    for i in 0..m - 1 {
        shift[target[i] as usize] = m - 1 - i;
    }

    let mut idx = 0;
    while idx + m <= n {
        // Compare backwards: path basename differs more frequently than prefix
        let mut j = m;
        while j > 0 && content[idx + j - 1] == target[j - 1] {
            j -= 1;
        }

        if j == 0 {
            let is_start_boundary =
                idx == 0 || content[idx - 1] == b'\n' || content[idx - 1] == b'\r';
            let after_idx = idx + m;
            let is_end_boundary =
                after_idx == n || content[after_idx] == b'\n' || content[after_idx] == b'\r';

            if is_start_boundary && is_end_boundary {
                let start = idx;
                let mut end = after_idx;
                if end < n && content[end] == b'\r' {
                    end += 1;
                }
                if end < n && content[end] == b'\n' {
                    end += 1;
                }
                return Some((start, end));
            }
        }

        let last_byte = content[idx + m - 1];
        idx += shift[last_byte as usize];
    }

    None
}

#[inline]
fn has_blank_lines(content: &[u8]) -> bool {
    content.starts_with(b"\n")
        || content.starts_with(b"\r\n")
        || content.windows(2).any(|w| w == b"\n\n")
        || content.windows(3).any(|w| w == b"\n\r\n")
}

#[cfg(target_os = "linux")]
#[inline]
fn advise_sequential(file: &File) {
    use std::os::unix::io::AsRawFd;
    extern "C" {
        fn posix_fadvise(
            fd: std::os::raw::c_int,
            offset: i64,
            len: i64,
            advice: std::os::raw::c_int,
        ) -> std::os::raw::c_int;
    }
    const POSIX_FADV_SEQUENTIAL: std::os::raw::c_int = 2;
    const POSIX_FADV_WILLNEED: std::os::raw::c_int = 3;

    let fd = file.as_raw_fd();
    unsafe {
        let _ = posix_fadvise(fd, 0, 0, POSIX_FADV_SEQUENTIAL | POSIX_FADV_WILLNEED);
    }
}

/// Writes all slices using writev(2) without buffer copying.
pub fn write_all_vectored<W: Write>(mut writer: W, mut bufs: &[IoSlice<'_>]) -> io::Result<()> {
    while !bufs.is_empty() {
        while !bufs.is_empty() && bufs[0].is_empty() {
            bufs = &bufs[1..];
        }
        if bufs.is_empty() {
            break;
        }

        let n = writer.write_vectored(bufs)?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "failed to write whole buffer",
            ));
        }

        let mut rem = n;
        while rem > 0 && !bufs.is_empty() {
            if bufs[0].len() <= rem {
                rem -= bufs[0].len();
                bufs = &bufs[1..];
            } else {
                let remaining = &bufs[0][rem..];
                writer.write_all(remaining)?;
                bufs = &bufs[1..];
                rem = 0;
            }
        }
    }
    Ok(())
}

fn read_file_optimized(path: &Path) -> io::Result<Option<(File, Vec<u8>)>> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };

    #[cfg(target_os = "linux")]
    advise_sequential(&file);

    let len = file.metadata()?.len() as usize;
    let mut buf = Vec::with_capacity(len);
    let mut reader = &file;
    reader.read_to_end(&mut buf)?;
    Ok(Some((file, buf)))
}

/// Prepends the path from reader to the log file without duplicates.
pub fn record_path<R: Read>(reader: R, log_path: &Path) -> io::Result<()> {
    let target = read_single_path(reader)?;
    if target.is_empty() {
        return Ok(());
    }
    let target = target.as_slice();

    if let Some(parent) = log_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    let (existing_file, content) = match read_file_optimized(log_path)? {
        Some((f, c)) => (Some(f), c),
        None => {
            let mut file = File::create(log_path)?;
            let slices = [IoSlice::new(target), IoSlice::new(b"\n")];
            write_all_vectored(&mut file, &slices)?;
            return Ok(());
        }
    };

    // Fast path: no-op if target is already the first line
    if let Some((start, _)) = find_exact_line(&content, target) {
        if start == 0 {
            return Ok(());
        }
    }

    #[cfg(unix)]
    let orig_permissions =
        existing_file.as_ref().and_then(|f| f.metadata().ok().map(|m| m.permissions()));

    let parent = log_path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = log_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("cdlog");
    let pid = std::process::id();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_path = parent.join(format!(".{}.tmp.{}_{}", file_name, pid, now));

    let guard = TmpFileGuard(&tmp_path);

    {
        let mut tmp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)?;

        #[cfg(unix)]
        if let Some(perms) = orig_permissions {
            let _ = fs::set_permissions(&tmp_path, perms);
        }

        if has_blank_lines(&content) {
            let mut writer = io::BufWriter::new(tmp_file);
            writer.write_all(target)?;
            writer.write_all(b"\n")?;
            for raw_line in content.split(|&b| b == b'\n') {
                let trimmed = trim_newlines(raw_line);
                if !trimmed.is_empty() && trimmed != target {
                    writer.write_all(trimmed)?;
                    writer.write_all(b"\n")?;
                }
            }
            writer.flush()?;
        } else {
            let nl = b"\n";
            let slices = if let Some((start, end)) = find_exact_line(&content, target) {
                [
                    IoSlice::new(target),
                    IoSlice::new(nl),
                    IoSlice::new(&content[..start]),
                    IoSlice::new(&content[end..]),
                ]
            } else {
                let needs_nl = !content.is_empty() && !content.ends_with(b"\n");
                [
                    IoSlice::new(target),
                    IoSlice::new(nl),
                    IoSlice::new(&content),
                    if needs_nl {
                        IoSlice::new(nl)
                    } else {
                        IoSlice::new(&[])
                    },
                ]
            };
            write_all_vectored(&mut tmp_file, &slices)?;
        }
    }

    fs::rename(&tmp_path, log_path)?;
    std::mem::forget(guard);

    Ok(())
}

/// Outputs recorded paths except the path provided in reader.
pub fn get_paths<R: Read, W: Write>(
    reader: R,
    mut writer: W,
    log_path: &Path,
) -> io::Result<()> {
    let exclude = read_single_path(reader)?;
    let exclude_target = exclude.as_slice();

    let content = match read_file_optimized(log_path)? {
        Some((_, c)) => c,
        None => return Ok(()),
    };

    if has_blank_lines(&content) {
        let mut buf_writer = io::BufWriter::new(writer);
        for raw_line in content.split(|&b| b == b'\n') {
            let trimmed = trim_newlines(raw_line);
            if !trimmed.is_empty() && (exclude_target.is_empty() || trimmed != exclude_target) {
                buf_writer.write_all(trimmed)?;
                buf_writer.write_all(b"\n")?;
            }
        }
        buf_writer.flush()?;
    } else if exclude_target.is_empty() {
        let slices = [IoSlice::new(&content)];
        write_all_vectored(&mut writer, &slices)?;
    } else if let Some((start, end)) = find_exact_line(&content, exclude_target) {
        let slices = [
            IoSlice::new(&content[..start]),
            IoSlice::new(&content[end..]),
        ];
        write_all_vectored(&mut writer, &slices)?;
    } else {
        let slices = [IoSlice::new(&content)];
        write_all_vectored(&mut writer, &slices)?;
    }

    Ok(())
}

pub fn run(args: impl IntoIterator<Item = OsString>) -> io::Result<()> {
    let config = match parse_args(args) {
        Ok(Some(c)) => c,
        Ok(None) => return Ok(()),
        Err(e) => {
            eprintln!("cdlog: {}", e);
            std::process::exit(1);
        }
    };

    let stdin = io::stdin();
    match config.mode {
        Mode::Record => {
            record_path(stdin.lock(), &config.log_path)?;
        }
        Mode::Get => {
            let stdout = io::stdout();
            get_paths(stdin.lock(), stdout.lock(), &config.log_path)?;
        }
    }

    Ok(())
}
