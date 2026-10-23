use cdlog::{get_paths, parse_args, record_path, Mode};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn temp_log_path(test_name: &str) -> PathBuf {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("cdlog_test_{}_{}", test_name, now));
    fs::create_dir_all(&dir).unwrap();
    dir.join(".cdlog")
}

#[test]
fn test_record_new_file() {
    let log_path = temp_log_path("new_file");
    record_path("/home/user/first\n".as_bytes(), &log_path).unwrap();

    let content = fs::read_to_string(&log_path).unwrap();
    assert_eq!(content, "/home/user/first\n");

    let _ = fs::remove_dir_all(log_path.parent().unwrap());
}

#[test]
fn test_record_creates_parent_dir() {
    let base = std::env::temp_dir().join(format!(
        "cdlog_test_parent_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let log_path = base.join("subdir").join("nested").join(".cdlog");
    record_path("/path/to/dir\n".as_bytes(), &log_path).unwrap();

    assert!(log_path.exists());
    let content = fs::read_to_string(&log_path).unwrap();
    assert_eq!(content, "/path/to/dir\n");

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn test_record_prepend_and_deduplicate() {
    let log_path = temp_log_path("dedup");

    record_path("/path/a\n".as_bytes(), &log_path).unwrap();
    record_path("/path/b\n".as_bytes(), &log_path).unwrap();
    record_path("/path/c\n".as_bytes(), &log_path).unwrap();

    let content = fs::read_to_string(&log_path).unwrap();
    assert_eq!(content, "/path/c\n/path/b\n/path/a\n");

    record_path("/path/b\n".as_bytes(), &log_path).unwrap();
    let content2 = fs::read_to_string(&log_path).unwrap();
    assert_eq!(content2, "/path/b\n/path/c\n/path/a\n");

    let _ = fs::remove_dir_all(log_path.parent().unwrap());
}

#[test]
fn test_record_noop_when_already_at_top() {
    let log_path = temp_log_path("noop");

    record_path("/path/a\n".as_bytes(), &log_path).unwrap();
    let metadata_before = fs::metadata(&log_path).unwrap();
    let modified_before = metadata_before.modified().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(50));
    record_path("/path/a\n".as_bytes(), &log_path).unwrap();

    let metadata_after = fs::metadata(&log_path).unwrap();
    let modified_after = metadata_after.modified().unwrap();

    assert_eq!(modified_before, modified_after);

    let content = fs::read_to_string(&log_path).unwrap();
    assert_eq!(content, "/path/a\n");

    let _ = fs::remove_dir_all(log_path.parent().unwrap());
}

#[test]
fn test_record_empty_input_does_nothing() {
    let log_path = temp_log_path("empty");

    record_path("".as_bytes(), &log_path).unwrap();
    assert!(!log_path.exists());

    record_path("\n".as_bytes(), &log_path).unwrap();
    assert!(!log_path.exists());

    let _ = fs::remove_dir_all(log_path.parent().unwrap());
}

#[cfg(unix)]
#[test]
fn test_record_preserves_permissions() {
    let log_path = temp_log_path("perms");

    {
        let mut file = File::create(&log_path).unwrap();
        file.write_all(b"/path/initial\n").unwrap();
    }
    let perms = fs::Permissions::from_mode(0o600);
    fs::set_permissions(&log_path, perms).unwrap();

    let initial_mode = fs::metadata(&log_path).unwrap().permissions().mode() & 0o777;
    assert_eq!(initial_mode, 0o600);

    record_path("/path/second\n".as_bytes(), &log_path).unwrap();

    let updated_mode = fs::metadata(&log_path).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        updated_mode, 0o600,
        "Permissions should remain 0600 after update"
    );

    let content = fs::read_to_string(&log_path).unwrap();
    assert_eq!(content, "/path/second\n/path/initial\n");

    let _ = fs::remove_dir_all(log_path.parent().unwrap());
}

#[test]
fn test_get_paths_excludes_current() {
    let log_path = temp_log_path("get");

    record_path("/path/1\n".as_bytes(), &log_path).unwrap();
    record_path("/path/2\n".as_bytes(), &log_path).unwrap();
    record_path("/path/3\n".as_bytes(), &log_path).unwrap();

    let mut out = Vec::new();
    get_paths("/path/2\n".as_bytes(), &mut out, &log_path).unwrap();
    let out_str = String::from_utf8(out).unwrap();

    assert_eq!(out_str, "/path/3\n/path/1\n");

    let _ = fs::remove_dir_all(log_path.parent().unwrap());
}

#[test]
fn test_get_paths_empty_exclude_returns_all() {
    let log_path = temp_log_path("get_all");

    record_path("/path/1\n".as_bytes(), &log_path).unwrap();
    record_path("/path/2\n".as_bytes(), &log_path).unwrap();

    let mut out = Vec::new();
    get_paths("".as_bytes(), &mut out, &log_path).unwrap();
    let out_str = String::from_utf8(out).unwrap();

    assert_eq!(out_str, "/path/2\n/path/1\n");

    let _ = fs::remove_dir_all(log_path.parent().unwrap());
}

#[test]
fn test_get_paths_nonexistent_file() {
    let log_path = PathBuf::from("/tmp/nonexistent_cdlog_123456");
    let mut out = Vec::new();
    get_paths("/path/current\n".as_bytes(), &mut out, &log_path).unwrap();
    assert!(out.is_empty());
}

#[test]
fn test_parse_args() {
    // Missing required argument
    assert!(parse_args(vec![OsString::from("cdlog")]).is_err());

    // Record mode with path
    let conf = parse_args(vec![
        OsString::from("cdlog"),
        OsString::from("/path/to/log"),
    ])
    .unwrap()
    .unwrap();
    assert_eq!(conf.mode, Mode::Record);
    assert_eq!(conf.log_path, PathBuf::from("/path/to/log"));

    // With -g flag
    let conf = parse_args(vec![
        OsString::from("cdlog"),
        OsString::from("-g"),
        OsString::from("/custom/path"),
    ])
    .unwrap()
    .unwrap();
    assert_eq!(conf.mode, Mode::Get);
    assert_eq!(conf.log_path, PathBuf::from("/custom/path"));

    // With --get flag
    let conf = parse_args(vec![
        OsString::from("cdlog"),
        OsString::from("/custom/path2"),
        OsString::from("--get"),
    ])
    .unwrap()
    .unwrap();
    assert_eq!(conf.mode, Mode::Get);
    assert_eq!(conf.log_path, PathBuf::from("/custom/path2"));
}
