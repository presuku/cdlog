use cdlog::{find_exact_line, get_paths, record_path};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

fn temp_log_path(test_name: &str) -> PathBuf {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("cdlog_edge_{}_{}", test_name, now));
    fs::create_dir_all(&dir).unwrap();
    dir.join(".cdlog")
}

// =========================================================================
// 1. Multibyte and Unicode edge cases
// =========================================================================
mod multibyte_and_unicode {
    use super::*;

    #[test]
    fn test_japanese_paths() {
        let log = temp_log_path("japanese");

        let path1 = "/home/ユーザー/ドキュメント/開発プロジェクト\n";
        let path2 = "/home/ユーザー/ダウンロード/写真・動画\n";
        let path3 = "/home/ユーザー/デスクトップ/ﾊﾝｶｸｶﾅと漢字\n";

        record_path(path1.as_bytes(), &log).unwrap();
        record_path(path2.as_bytes(), &log).unwrap();
        record_path(path3.as_bytes(), &log).unwrap();

        let content = fs::read_to_string(&log).unwrap();
        assert_eq!(
            content,
            format!("{}{}{}", path3, path2, path1)
        );

        let mut out = Vec::new();
        get_paths(path2.as_bytes(), &mut out, &log).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            format!("{}{}", path3, path1)
        );

        let _ = fs::remove_dir_all(log.parent().unwrap());
    }

    #[test]
    fn test_emoji_and_combining_characters() {
        let log = temp_log_path("emoji");

        let path_crab = "/workspace/🦀-rust-project\n";
        let path_family = "/workspace/👨‍👩‍👧‍👦-family-photos\n";
        let path_nfc = "/workspace/がっこう\n";
        let path_nfd = "/workspace/か\u{3099}っこう\n";

        record_path(path_crab.as_bytes(), &log).unwrap();
        record_path(path_family.as_bytes(), &log).unwrap();
        record_path(path_nfc.as_bytes(), &log).unwrap();
        record_path(path_nfd.as_bytes(), &log).unwrap();

        let content = fs::read_to_string(&log).unwrap();
        assert_eq!(
            content,
            format!("{}{}{}{}", path_nfd, path_nfc, path_family, path_crab)
        );

        record_path(path_crab.as_bytes(), &log).unwrap();
        let content_after = fs::read_to_string(&log).unwrap();
        assert_eq!(
            content_after,
            format!("{}{}{}{}", path_crab, path_nfd, path_nfc, path_family)
        );

        let _ = fs::remove_dir_all(log.parent().unwrap());
    }

    #[test]
    fn test_multilingual_cjk_rtl_accents() {
        let log = temp_log_path("multilingual");

        let cjk_simplified = "/path/简体中文目录\n";
        let cjk_traditional = "/path/繁體中文目錄\n";
        let hangul = "/path/한국어_디렉토리\n";
        let cyrillic = "/path/русский_каталог\n";
        let rtl_arabic = "/path/دليل_عربي\n";
        let accented = "/path/café_naïve_résumé\n";

        let paths = [
            cjk_simplified,
            cjk_traditional,
            hangul,
            cyrillic,
            rtl_arabic,
            accented,
        ];

        for p in &paths {
            record_path(p.as_bytes(), &log).unwrap();
        }

        let mut expected = String::new();
        for p in paths.iter().rev() {
            expected.push_str(p);
        }
        assert_eq!(fs::read_to_string(&log).unwrap(), expected);

        let _ = fs::remove_dir_all(log.parent().unwrap());
    }
}

// =========================================================================
// 2. Arbitrary bytes (0x01-0xFF, non-UTF-8, control characters)
// =========================================================================
mod arbitrary_bytes {
    use super::*;

    #[test]
    fn test_invalid_utf8_byte_sequences() {
        let log = temp_log_path("invalid_utf8");

        let invalid_path1: &[u8] = b"/invalid/\xff\xfe\xfd/path\n";
        let invalid_path2: &[u8] = b"/corrupted/\xc0\xaf/test\n";
        let valid_path: &[u8] = b"/normal/path\n";

        record_path(invalid_path1, &log).unwrap();
        record_path(invalid_path2, &log).unwrap();
        record_path(valid_path, &log).unwrap();

        let raw_content = fs::read(&log).unwrap();
        let mut expected = Vec::new();
        expected.extend_from_slice(valid_path);
        expected.extend_from_slice(invalid_path2);
        expected.extend_from_slice(invalid_path1);
        assert_eq!(raw_content, expected);

        let mut out = Vec::new();
        get_paths(invalid_path2, &mut out, &log).unwrap();
        let mut expected_get = Vec::new();
        expected_get.extend_from_slice(valid_path);
        expected_get.extend_from_slice(invalid_path1);
        assert_eq!(out, expected_get);

        let _ = fs::remove_dir_all(log.parent().unwrap());
    }

    #[test]
    fn test_control_characters_in_bytes() {
        let log = temp_log_path("control_chars");

        let ctrl_path1: &[u8] = b"/path/with\x09tab\n";
        let ctrl_path2: &[u8] = b"/path/with\x1b[31mcolor\x1b[0m\n";
        let ctrl_path3: &[u8] = b"/path/with\x07bell\x08bs\x7fdel\n";

        record_path(ctrl_path1, &log).unwrap();
        record_path(ctrl_path2, &log).unwrap();
        record_path(ctrl_path3, &log).unwrap();

        let raw = fs::read(&log).unwrap();
        let mut expected = Vec::new();
        expected.extend_from_slice(ctrl_path3);
        expected.extend_from_slice(ctrl_path2);
        expected.extend_from_slice(ctrl_path1);
        assert_eq!(raw, expected);

        let _ = fs::remove_dir_all(log.parent().unwrap());
    }

    #[test]
    fn test_all_possible_bytes_except_newlines() {
        let log = temp_log_path("all_bytes");

        let mut all_bytes = Vec::new();
        all_bytes.extend_from_slice(b"/prefix/");
        for b in 1u8..=255u8 {
            if b != b'\n' && b != b'\r' {
                all_bytes.push(b);
            }
        }
        all_bytes.push(b'\n');

        record_path(&all_bytes[..], &log).unwrap();

        let raw = fs::read(&log).unwrap();
        assert_eq!(raw, all_bytes);

        record_path(&all_bytes[..], &log).unwrap();
        let raw2 = fs::read(&log).unwrap();
        assert_eq!(raw2, all_bytes);

        let mut out = Vec::new();
        get_paths(&all_bytes[..], &mut out, &log).unwrap();
        assert!(out.is_empty());

        let _ = fs::remove_dir_all(log.parent().unwrap());
    }

    #[test]
    fn test_embedded_null_byte() {
        let log = temp_log_path("embedded_null");

        let null_path: &[u8] = b"/path/with\x00null/inside\n";
        let normal_path: &[u8] = b"/path/normal\n";

        record_path(null_path, &log).unwrap();
        record_path(normal_path, &log).unwrap();

        let mut out = Vec::new();
        get_paths(normal_path, &mut out, &log).unwrap();
        assert_eq!(out, null_path);

        let _ = fs::remove_dir_all(log.parent().unwrap());
    }
}

// =========================================================================
// 3. Special shell characters and quotes
// =========================================================================
mod special_shell_characters {
    use super::*;

    #[test]
    fn test_quotes_and_expansion_characters() {
        let log = temp_log_path("shell_chars");

        let p1 = "/path/with 'single' and \"double\" and `backtick`\n";
        let p2 = "/path/with $HOME and ${VAR} and $(command)\n";
        let p3 = "/path/with \\backslashes\\and/slashes\n";
        let p4 = "/path/with | pipe & ampersand ; semicolon\n";
        let p5 = "/path/with * star ? question [bracket] {brace} (paren)\n";
        let p6 = "/path/with ~ tilde ! exclamation # hash % percent ^ caret\n";

        let paths = [p1, p2, p3, p4, p5, p6];
        for p in &paths {
            record_path(p.as_bytes(), &log).unwrap();
        }

        record_path(p2.as_bytes(), &log).unwrap();

        let content = fs::read_to_string(&log).unwrap();
        assert!(content.starts_with(p2));
        assert_eq!(content.matches(p2.trim_end()).count(), 1);

        let mut out = Vec::new();
        get_paths(p2.as_bytes(), &mut out, &log).unwrap();
        let out_str = String::from_utf8(out).unwrap();
        assert!(!out_str.contains(p2.trim_end()));
        assert!(out_str.contains(p1.trim_end()));
        assert!(out_str.contains(p3.trim_end()));

        let _ = fs::remove_dir_all(log.parent().unwrap());
    }

    #[test]
    fn test_spaces_and_leading_trailing_tabs() {
        let log = temp_log_path("spaces");

        let p_spaces = "/path/with   multiple    consecutive     spaces\n";
        let p_tab = "/path/with\ttab\tseparators\n";
        let p_leading_space = " /leading/space/path\n";

        record_path(p_spaces.as_bytes(), &log).unwrap();
        record_path(p_tab.as_bytes(), &log).unwrap();
        record_path(p_leading_space.as_bytes(), &log).unwrap();

        let content = fs::read_to_string(&log).unwrap();
        assert_eq!(
            content,
            format!("{}{}{}", p_leading_space, p_tab, p_spaces)
        );

        let _ = fs::remove_dir_all(log.parent().unwrap());
    }
}

// =========================================================================
// 4. Boundary conditions and structural edge cases
// =========================================================================
mod structural_and_boundaries {
    use super::*;

    #[test]
    fn test_exact_match_vs_prefix_or_substring() {
        let log = temp_log_path("prefix_match");

        let p_short = "/dir\n";
        let p_mid = "/dir/sub\n";
        let p_long = "/directory\n";
        let p_other = "/other/dir\n";

        record_path(p_short.as_bytes(), &log).unwrap();
        record_path(p_mid.as_bytes(), &log).unwrap();
        record_path(p_long.as_bytes(), &log).unwrap();
        record_path(p_other.as_bytes(), &log).unwrap();

        let mut out = Vec::new();
        get_paths(p_short.as_bytes(), &mut out, &log).unwrap();
        let out_str = String::from_utf8(out).unwrap();

        assert_eq!(
            out_str,
            format!("{}{}{}", p_other, p_long, p_mid)
        );

        let _ = fs::remove_dir_all(log.parent().unwrap());
    }

    #[test]
    fn test_crlf_and_lf_mixed_inputs() {
        let log = temp_log_path("crlf");

        let p1 = "/path/one\r\n";
        let p2 = "/path/two\n";

        record_path(p1.as_bytes(), &log).unwrap();
        record_path(p2.as_bytes(), &log).unwrap();

        let content = fs::read_to_string(&log).unwrap();
        assert_eq!(content, "/path/two\n/path/one\n");

        let mut out = Vec::new();
        get_paths("/path/two\r\n".as_bytes(), &mut out, &log).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "/path/one\n");

        let _ = fs::remove_dir_all(log.parent().unwrap());
    }

    #[test]
    fn test_no_trailing_newline_on_stdin() {
        let log = temp_log_path("no_newline");

        record_path("/path/without/newline".as_bytes(), &log).unwrap();

        let content = fs::read_to_string(&log).unwrap();
        assert_eq!(content, "/path/without/newline\n");

        let mut out = Vec::new();
        get_paths("/path/without/newline".as_bytes(), &mut out, &log).unwrap();
        assert!(out.is_empty());

        let _ = fs::remove_dir_all(log.parent().unwrap());
    }

    #[test]
    fn test_very_long_path() {
        let log = temp_log_path("long_path");

        let mut long_dir = String::from("/root");
        for _ in 0..800 {
            long_dir.push_str("/abcdefghij");
        }
        long_dir.push('\n');

        record_path(long_dir.as_bytes(), &log).unwrap();
        record_path("/short/path\n".as_bytes(), &log).unwrap();

        let content = fs::read_to_string(&log).unwrap();
        assert_eq!(
            content,
            format!("/short/path\n{}", long_dir)
        );

        record_path(long_dir.as_bytes(), &log).unwrap();
        let content2 = fs::read_to_string(&log).unwrap();
        assert_eq!(
            content2,
            format!("{}/short/path\n", long_dir)
        );

        let _ = fs::remove_dir_all(log.parent().unwrap());
    }

    #[test]
    fn test_single_character_paths() {
        let log = temp_log_path("single_char");

        record_path("/\n".as_bytes(), &log).unwrap();
        record_path("/a\n".as_bytes(), &log).unwrap();
        record_path("/b\n".as_bytes(), &log).unwrap();

        let content = fs::read_to_string(&log).unwrap();
        assert_eq!(content, "/b\n/a\n/\n");

        let _ = fs::remove_dir_all(log.parent().unwrap());
    }

    #[test]
    fn test_existing_file_with_blank_lines() {
        let log = temp_log_path("blank_lines");

        {
            let mut f = File::create(&log).unwrap();
            f.write_all(b"\n/path/old1\n\n\n/path/old2\n\n").unwrap();
        }

        record_path("/path/new\n".as_bytes(), &log).unwrap();

        let content = fs::read_to_string(&log).unwrap();
        assert_eq!(content, "/path/new\n/path/old1\n/path/old2\n");

        let _ = fs::remove_dir_all(log.parent().unwrap());
    }

    #[test]
    fn test_many_entries_scalability() {
        let log = temp_log_path("many_entries");

        {
            let mut f = File::create(&log).unwrap();
            for i in 0..5000 {
                writeln!(f, "/path/dir_{}", i).unwrap();
            }
        }

        let target = "/path/dir_2500\n";
        record_path(target.as_bytes(), &log).unwrap();

        let f = File::open(&log).unwrap();
        use std::io::BufRead;
        let reader = std::io::BufReader::new(f);
        let mut lines = reader.lines();

        assert_eq!(lines.next().unwrap().unwrap(), "/path/dir_2500");

        let total_count = 1 + lines.count();
        assert_eq!(total_count, 5000);

        let _ = fs::remove_dir_all(log.parent().unwrap());
    }
}

// =========================================================================
// 5. Boyer-Moore-Horspool search unit tests
// =========================================================================
mod search_algorithm {
    use super::*;

    #[test]
    fn test_find_exact_line_positions() {
        let text = b"/first/line\n/second/target/path\n/third/line\n";

        let (start, end) = find_exact_line(text, b"/second/target/path").unwrap();
        assert_eq!(&text[start..end], b"/second/target/path\n");

        let (start, end) = find_exact_line(text, b"/first/line").unwrap();
        assert_eq!(start, 0);
        assert_eq!(&text[start..end], b"/first/line\n");

        let (start, end) = find_exact_line(text, b"/third/line").unwrap();
        assert_eq!(&text[start..end], b"/third/line\n");

        assert!(find_exact_line(text, b"/nonexistent").is_none());
    }

    #[test]
    fn test_find_exact_line_boundary_safety() {
        let text = b"/a/b/c/d\n/prefix/a/b\n/a/b/suffix\n";

        assert!(find_exact_line(text, b"/a/b").is_none());

        let text_with_exact = b"/a/b/c/d\n/a/b\n/a/b/suffix\n";
        let (start, end) = find_exact_line(text_with_exact, b"/a/b").unwrap();
        assert_eq!(&text_with_exact[start..end], b"/a/b\n");
    }

    #[test]
    fn test_find_exact_line_crlf() {
        let text = b"/first\r\n/second/path\r\n/third\r\n";
        let (start, end) = find_exact_line(text, b"/second/path").unwrap();
        assert_eq!(&text[start..end], b"/second/path\r\n");
    }

    #[test]
    fn test_find_exact_line_no_trailing_newline() {
        let text = b"/first\n/last/line/without/nl";
        let (start, end) = find_exact_line(text, b"/last/line/without/nl").unwrap();
        assert_eq!(&text[start..end], b"/last/line/without/nl");
        assert_eq!(end, text.len());
    }
}
