pub mod config;

use cliffa::cli::AppHandle;
pub use config::*;
use miette::{Context, IntoDiagnostic, Result};
use rustc_hash::FxHashMap;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tracing::info;

/// Count per-key character frequencies (letters, digits, punctuation) across all
/// files under `input` matching any of the masks. Shifted chars fold into their
/// unshifted key (`A`→`a`, `!`→`1`, `:`→`;`, …); whitespace and non-ASCII are skipped.
pub fn frequencies(cfg: FrequenciesConfig, app: AppHandle) -> Result<()> {
    let input = cfg
        .input
        .wrap_err("Frequencies mode requires `frequencies.input` folder")?;

    let paths = collect_files(&input, &cfg.masks);
    info!(folder = %input.display(), count = paths.len(), "Counting key frequencies");

    let mut counts: FxHashMap<char, u64> = FxHashMap::default();
    for path in &paths {
        if app.should_finish() {
            info!("Frequencies interrupted");
            return Ok(());
        }
        count_file(path, &mut counts);
    }

    let rows = sorted_rows(&counts);
    print_rows(&rows, cfg.print);

    if let Some(output) = &cfg.output {
        write_csv(output, &rows)?;
        info!("Results written to {}", output.display());
    }

    Ok(())
}

/// Map a char to its physical key: shifted variants fold into the unshifted char
/// (US layout). `None` for whitespace, control, and non-ASCII.
fn base_key(c: char) -> Option<char> {
    match c {
        c if c.is_ascii_alphabetic() => Some(c.to_ascii_lowercase()),
        '!' => Some('1'),
        '@' => Some('2'),
        '#' => Some('3'),
        '$' => Some('4'),
        '%' => Some('5'),
        '^' => Some('6'),
        '&' => Some('7'),
        '*' => Some('8'),
        '(' => Some('9'),
        ')' => Some('0'),
        '_' => Some('-'),
        '+' => Some('='),
        '{' => Some('['),
        '}' => Some(']'),
        '|' => Some('\\'),
        ':' => Some(';'),
        '"' => Some('\''),
        '<' => Some(','),
        '>' => Some('.'),
        '?' => Some('/'),
        '~' => Some('`'),
        c if c.is_ascii_graphic() => Some(c), // digits + unshifted punctuation
        _ => None,
    }
}

/// Shifted glyph sharing the key (US layout): `.`→`>`, `1`→`!`, …
/// `None` for letters (case-folded) and keys without a pair.
fn shifted(key: char) -> Option<char> {
    match key {
        '1' => Some('!'),
        '2' => Some('@'),
        '3' => Some('#'),
        '4' => Some('$'),
        '5' => Some('%'),
        '6' => Some('^'),
        '7' => Some('&'),
        '8' => Some('*'),
        '9' => Some('('),
        '0' => Some(')'),
        '-' => Some('_'),
        '=' => Some('+'),
        '[' => Some('{'),
        ']' => Some('}'),
        '\\' => Some('|'),
        ';' => Some(':'),
        '\'' => Some('"'),
        ',' => Some('<'),
        '.' => Some('>'),
        '/' => Some('?'),
        '`' => Some('~'),
        _ => None,
    }
}

/// Display label for a key: both glyphs for shift pairs (`.>`), the bare char otherwise.
fn key_label(key: char) -> String {
    match shifted(key) {
        Some(s) => format!("{key}{s}"),
        None => key.to_string(),
    }
}

/// Quote a CSV field when it contains a comma or quote (quotes doubled).
fn csv_field(s: &str) -> String {
    if s.contains([',', '"']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// `true` when `name` matches `mask` (`*` = any run, `?` = any single char);
/// both compared case-insensitively.
fn mask_match(mask: &str, name: &str) -> bool {
    fn rec(m: &[u8], n: &[u8]) -> bool {
        match (m.first(), n.first()) {
            (None, None) => true,
            (Some(b'*'), _) => rec(&m[1..], n) || (!n.is_empty() && rec(m, &n[1..])),
            (Some(b'?'), Some(_)) => rec(&m[1..], &n[1..]),
            (Some(a), Some(b)) => a == b && rec(&m[1..], &n[1..]),
            _ => false,
        }
    }
    rec(
        mask.to_ascii_lowercase().as_bytes(),
        name.to_ascii_lowercase().as_bytes(),
    )
}

/// Recursively collect files under `dir` whose names match any mask (all when empty).
/// Unreadable folders are skipped with a warning.
fn collect_files(dir: &Path, masks: &[String]) -> Vec<PathBuf> {
    let matches = |name: &str| masks.is_empty() || masks.iter().any(|m| mask_match(m, name));
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!(folder = %dir.display(), error = %e, "Skipping unreadable folder");
                continue;
            }
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(matches)
            {
                files.push(path);
            }
        }
    }

    files.sort();
    files
}

/// Fold one file's bytes into `counts` (non-ASCII bytes fall out via `base_key`).
/// Unreadable files are skipped with a warning.
fn count_file(path: &Path, counts: &mut FxHashMap<char, u64>) {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::warn!(file = %path.display(), error = %e, "Skipping unreadable file");
            return;
        }
    };

    for key in bytes.iter().filter_map(|&b| base_key(b as char)) {
        *counts.entry(key).or_insert(0) += 1;
    }
}

/// Rows sorted by count descending: `(key, count, frequency)`.
fn sorted_rows(counts: &FxHashMap<char, u64>) -> Vec<(char, u64, f64)> {
    let total: u64 = counts.values().sum::<u64>().max(1);
    let mut rows: Vec<_> = counts
        .iter()
        .map(|(&key, &count)| (key, count, count as f64 / total as f64))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    rows
}

/// Print the top `to_print` keys.
fn print_rows(rows: &[(char, u64, f64)], to_print: usize) {
    for (key, count, freq) in rows.iter().take(to_print) {
        println!("{:2} {:6.3}% {count}", key_label(*key), freq * 100.0);
    }
}

/// Write all rows as `key,count,frequency` CSV (keys with a shift pair as `.>`,
/// comma-containing labels quoted).
fn write_csv(path: &Path, rows: &[(char, u64, f64)]) -> Result<()> {
    let mut file = fs::File::create(path)
        .into_diagnostic()
        .wrap_err("Failed to create frequencies output")?;
    writeln!(file, "key,count,frequency").into_diagnostic()?;
    for (key, count, freq) in rows {
        let label = csv_field(&key_label(*key));
        writeln!(file, "{label},{count},{freq:.6}").into_diagnostic()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_key_folds_shift_pairs() {
        assert_eq!(base_key('A'), Some('a'));
        assert_eq!(base_key('!'), Some('1'));
        assert_eq!(base_key(':'), Some(';'));
        assert_eq!(base_key('"'), Some('\''));
        assert_eq!(base_key('~'), Some('`'));
        assert_eq!(base_key('?'), Some('/'));
    }

    #[test]
    fn base_key_keeps_unshifted_chars() {
        assert_eq!(base_key('a'), Some('a'));
        assert_eq!(base_key('7'), Some('7'));
        assert_eq!(base_key('.'), Some('.'));
        assert_eq!(base_key('-'), Some('-'));
    }

    #[test]
    fn base_key_skips_whitespace_and_non_ascii() {
        assert_eq!(base_key(' '), None);
        assert_eq!(base_key('\t'), None);
        assert_eq!(base_key('\n'), None);
        assert_eq!(base_key('é'), None);
    }

    #[test]
    fn key_label_shows_both_glyphs_for_shift_pairs() {
        assert_eq!(key_label('.'), ".>");
        assert_eq!(key_label('1'), "1!");
        assert_eq!(key_label(';'), ";:");
        assert_eq!(key_label(','), ",<");
        assert_eq!(key_label('a'), "a");
    }

    #[test]
    fn csv_field_quotes_commas_and_quotes() {
        assert_eq!(csv_field(",<"), "\",<\"");
        assert_eq!(csv_field("'\""), "\"'\"\"\"");
        assert_eq!(csv_field(".>"), ".>");
    }

    #[test]
    fn mask_match_wildcards() {
        assert!(mask_match("*.rs", "main.rs"));
        assert!(mask_match("*.RS", "main.rs"));
        assert!(!mask_match("*.rs", "main.rss"));
        assert!(mask_match("data?.csv", "data1.csv"));
        assert!(!mask_match("data?.csv", "data12.csv"));
        assert!(mask_match("*", "anything"));
    }

    #[test]
    fn sorted_rows_orders_by_count_desc() {
        let counts: FxHashMap<char, u64> = [('a', 1), ('b', 3), ('c', 3)].into_iter().collect();

        let rows = sorted_rows(&counts);

        assert_eq!(
            rows,
            vec![
                ('b', 3, 3.0 / 7.0),
                ('c', 3, 3.0 / 7.0),
                ('a', 1, 1.0 / 7.0),
            ]
        );
    }

    #[test]
    fn count_file_combines_shifted_and_unshifted() {
        let dir = std::env::temp_dir().join("keyvolve-freq-test");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sample.txt");
        fs::write(&path, "Aa! 1:;").unwrap();

        let mut counts = FxHashMap::default();
        count_file(&path, &mut counts);
        fs::remove_file(&path).ok();

        assert_eq!(counts[&'a'], 2); // A + a
        assert_eq!(counts[&'1'], 2); // ! + 1
        assert_eq!(counts[&';'], 2); // : + ;
    }
}
