use crate::app::synthesise::counter::CorpusScore;
use miette::{Context, IntoDiagnostic, Result};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

/// Write space-separated words to a text file.
pub(super) fn write_corpus(words: &[String], path: &Path) -> Result<()> {
    let mut out = fs::File::create(path)
        .into_diagnostic()
        .wrap_err("Failed to create corpus output file")?;
    for (i, word) in words.iter().enumerate() {
        if i > 0 {
            out.write_all(b" ").into_diagnostic()?;
        }
        out.write_all(word.as_bytes()).into_diagnostic()?;
    }
    out.write_all(b"\n").into_diagnostic()?;
    Ok(())
}

/// Report path next to corpus output.
pub(super) fn report_path(output: &Path) -> PathBuf {
    let stem = output
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    output
        .parent()
        .unwrap_or(output)
        .join(format!("{stem}.synth-report.txt"))
}

/// Write compact synth score report.
pub(super) fn write_report(
    path: &Path,
    score: &CorpusScore,
    source_words: usize,
    generated_words: usize,
    tolerance: f64,
) -> Result<()> {
    let mut out = fs::File::create(path)
        .into_diagnostic()
        .wrap_err("Failed to create synth report")?;
    writeln!(out, "source_words={source_words}").into_diagnostic()?;
    writeln!(out, "generated_words={generated_words}").into_diagnostic()?;
    writeln!(out, "tolerance={:.2}%", tolerance * 100.0).into_diagnostic()?;
    writeln!(out, "letters_error={:.2}%", score.letters * 100.0).into_diagnostic()?;
    writeln!(out, "bigrams_error={:.2}%", score.bigrams * 100.0).into_diagnostic()?;
    writeln!(
        out,
        "first_letters_error={:.2}%",
        score.first_letters * 100.0
    )
    .into_diagnostic()?;
    writeln!(
        out,
        "average_word_length_error={:.2}%",
        score.average_word_length * 100.0
    )
    .into_diagnostic()?;
    writeln!(out, "max_error={:.2}%", score.max_error * 100.0).into_diagnostic()?;
    writeln!(out, "passed={}", score.max_error <= tolerance).into_diagnostic()?;
    Ok(())
}
