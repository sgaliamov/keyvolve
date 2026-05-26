pub mod config;

use cliffa::cli::AppHandle;
pub use config::*;
use miette::{Context, IntoDiagnostic, Result};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

/// Merge all `.txt` files in a folder into one file.
/// Non-`a-z` chars (after lowercasing) become spaces; consecutive spaces on a line collapse to one.
/// Files are processed in parallel; results are written in sorted filename order.
pub fn merge(cfg: MergeConfig, app: AppHandle) -> Result<()> {
    let print = cfg.print;
    let shuffle = cfg.shuffle;
    let seed = cfg.seed;

    let input = cfg
        .input
        .wrap_err("Merge mode requires `merge.input` path")?;
    let output = cfg
        .output
        .wrap_err("Merge mode requires `merge.output` path")?;
    let output_path = output.canonicalize().unwrap_or_else(|_| output.clone());

    // Collect sorted .txt paths.
    let mut paths: Vec<PathBuf> = fs::read_dir(&input)
        .into_diagnostic()
        .wrap_err("Failed to read input folder")?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("txt"))
        .filter(|path| path.canonicalize().unwrap_or_else(|_| path.clone()) != output_path)
        .collect();
    paths.sort();

    tracing::info!(folder = %input.display(), count = paths.len(), "Merging files");

    let temp_dir = prepare_temp_dir(&output)?;
    let bucket_count = 256usize;
    let preview = spill_words(&paths, &temp_dir, bucket_count, shuffle, seed, print, &app)?;

    if app.should_finish() {
        tracing::info!("Merge interrupted before printing or writing");
        cleanup_temp_dir(&temp_dir);
        return Ok(());
    }

    print_words(&preview);
    write_buckets(&output, &temp_dir, bucket_count, shuffle, seed, &app)?;
    cleanup_temp_dir(&temp_dir);

    tracing::info!(output = %output.display(), "Merge complete");
    Ok(())
}

/// Stream cleaned words into temp buckets and collect preview words.
fn spill_words(
    paths: &[PathBuf],
    temp_dir: &Path,
    bucket_count: usize,
    shuffle: bool,
    seed: Option<u64>,
    print: usize,
    app: &AppHandle,
) -> Result<Vec<String>> {
    let mut writers = (0..bucket_count)
        .map(|i| {
            File::create(bucket_path(temp_dir, i))
                .into_diagnostic()
                .map(BufWriter::new)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut preview = Vec::with_capacity(print);
    let mut rng = make_rng(seed);
    let mut seen = 0usize;

    for path in paths {
        if app.should_finish() {
            break;
        }

        process_file(path, app, |word| {
            seen += 1;

            if preview.len() < print {
                preview.push(word.to_owned());
            } else if shuffle {
                let index = rng.random_range(0..seen);
                if index < print {
                    preview[index] = word.to_owned();
                }
            }

            let bucket = if shuffle {
                rng.random_range(0..bucket_count)
            } else {
                0
            };
            let writer = &mut writers[bucket];
            writer.write_all(word.as_bytes()).into_diagnostic()?;
            writer.write_all(b"\n").into_diagnostic()?;
            Ok(())
        })?;
    }

    for writer in &mut writers {
        writer.flush().into_diagnostic()?;
    }

    Ok(preview)
}

/// Print preview words to stdout.
fn print_words(words: &[String]) {
    for word in words {
        println!("{word}");
    }
}

/// Read bucket files and write final output.
fn write_buckets(
    output: &Path,
    temp_dir: &Path,
    bucket_count: usize,
    shuffle: bool,
    seed: Option<u64>,
    app: &AppHandle,
) -> Result<()> {
    let out_file = File::create(output)
        .into_diagnostic()
        .wrap_err("Failed to create output file")?;
    let mut writer = BufWriter::new(out_file);
    let mut order = (0..bucket_count).collect::<Vec<_>>();

    if shuffle {
        shuffle_indices(&mut order, seed, app);
    }

    for bucket in order {
        if app.should_finish() {
            tracing::info!("Merge interrupted while writing output");
            return Ok(());
        }

        write_bucket(
            &mut writer,
            &bucket_path(temp_dir, bucket),
            shuffle,
            seed,
            bucket as u64,
            app,
        )?;
    }

    writer.flush().into_diagnostic()?;
    Ok(())
}

/// Write one bucket, optionally shuffling its words in memory.
fn write_bucket(
    writer: &mut BufWriter<File>,
    path: &Path,
    shuffle: bool,
    seed: Option<u64>,
    salt: u64,
    app: &AppHandle,
) -> Result<()> {
    let file = File::open(path).into_diagnostic()?;
    let reader = BufReader::new(file);

    if shuffle {
        let mut words = Vec::new();
        for line in reader.lines() {
            if app.should_finish() {
                return Ok(());
            }

            words.push(line.into_diagnostic()?);
        }

        shuffle_words(&mut words, mix_seed(seed, salt), app);
        for word in &words {
            if app.should_finish() {
                return Ok(());
            }

            writer.write_all(word.as_bytes()).into_diagnostic()?;
            writer.write_all(b"\n").into_diagnostic()?;
        }
        return Ok(());
    }

    for line in reader.lines() {
        if app.should_finish() {
            return Ok(());
        }

        let word = line.into_diagnostic()?;
        writer.write_all(word.as_bytes()).into_diagnostic()?;
        writer.write_all(b"\n").into_diagnostic()?;
    }

    Ok(())
}

/// Read a file line by line, clean each line, then visit words.
fn process_file(
    path: &PathBuf,
    app: &AppHandle,
    mut on_word: impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    let file = File::open(path)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to open {}", path.display()))?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        if app.should_finish() {
            break;
        }

        let raw = line.into_diagnostic()?;
        let cleaned = clean_line(&raw);
        for word in cleaned.split_whitespace() {
            on_word(word)?;
        }
    }

    Ok(())
}

/// Shuffle words in place. Seeded when provided.
fn shuffle_words(words: &mut [String], seed: Option<u64>, app: &AppHandle) {
    let mut rng = make_rng(seed);

    for i in (1..words.len()).rev() {
        if app.should_finish() {
            tracing::info!("Shuffle interrupted");
            return;
        }

        let j = rng.random_range(0..=i);
        words.swap(i, j);
    }
}

/// Shuffle indices in place. Seeded when provided.
fn shuffle_indices(indices: &mut [usize], seed: Option<u64>, app: &AppHandle) {
    let mut rng = make_rng(seed);

    for i in (1..indices.len()).rev() {
        if app.should_finish() {
            tracing::info!("Shuffle interrupted");
            return;
        }

        let j = rng.random_range(0..=i);
        indices.swap(i, j);
    }
}

/// Create RNG from optional seed.
fn make_rng(seed: Option<u64>) -> StdRng {
    match seed {
        Some(seed) => StdRng::seed_from_u64(seed),
        None => {
            let mut rng = rand::rng();
            StdRng::from_rng(&mut rng)
        }
    }
}

/// Mix optional seed with a salt.
fn mix_seed(seed: Option<u64>, salt: u64) -> Option<u64> {
    seed.map(|seed| seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

/// Prepare temp directory next to output.
fn prepare_temp_dir(output: &Path) -> Result<PathBuf> {
    let temp_dir = output.with_extension("merge.tmp");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)
            .into_diagnostic()
            .wrap_err("Failed to clear merge temp directory")?;
    }
    fs::create_dir_all(&temp_dir)
        .into_diagnostic()
        .wrap_err("Failed to create merge temp directory")?;
    Ok(temp_dir)
}

/// Best-effort temp dir cleanup.
fn cleanup_temp_dir(temp_dir: &Path) {
    let _ = fs::remove_dir_all(temp_dir);
}

/// Temp bucket file path.
fn bucket_path(temp_dir: &Path, bucket: usize) -> PathBuf {
    temp_dir.join(format!("bucket-{bucket:03}.txt"))
}

/// Lowercase a-z only; everything else → space; collapse consecutive spaces.
fn clean_line(line: &str) -> String {
    let mut cleaned = String::with_capacity(line.len());
    let mut last_was_space = true;

    for byte in line.bytes() {
        let lower = byte.to_ascii_lowercase();
        let ch = if lower.is_ascii_lowercase() {
            lower as char
        } else {
            ' '
        };

        if ch == ' ' {
            if !last_was_space {
                cleaned.push(' ');
                last_was_space = true;
            }
        } else {
            cleaned.push(ch);
            last_was_space = false;
        }
    }

    if cleaned.ends_with(' ') {
        cleaned.pop();
    }

    cleaned
}
