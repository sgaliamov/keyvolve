pub mod config;
mod output;
mod select;
mod state;

use crate::models::Keyboard;
use cliffa::cli::AppHandle;
pub use config::*;
use miette::{Context, Result};
pub use output::*;
use rand::{RngExt, SeedableRng, rngs::StdRng};
pub use select::*;
pub use state::*;
use std::io::{BufRead, Write};
use std::path::Path;

/// Interactive pair-ranking mode: repeatedly asks the user which of two
/// bigram pairs is easier to type, refining Glicko-lite ratings for all 210
/// ordered left-hand pairs. Resumable; writes ranked keyboard JSON + CSV report.
pub fn rank(cfg: RankConfig, keyboard_path: impl AsRef<Path>, app: AppHandle) -> Result<()> {
    let keyboard = Keyboard::load(keyboard_path).wrap_err("Rank mode needs a keyboard file")?;
    let session = cfg.session_path();
    let mut state = RankState::load_or_new(&session)?;
    let mut rng = match cfg.seed {
        Some(seed) => StdRng::seed_from_u64(seed),
        None => StdRng::from_rng(&mut rand::rng()),
    };

    println!("Rank mode: type the pair on your QWERTY keyboard, pick the EASIER one.");
    println!("Answers: 1 / 2 = winner, = tie, u undo, s stats, q quit (state is saved).");
    if state.finished {
        println!("Ranking finished earlier — verification mode: checking saved ranking.");
    }

    // Verification counters for this run.
    let (mut confirmed, mut contradicted) = (0u32, 0u32);

    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();

    while !app.should_finish() {
        let total = state.items.len();
        let settled = state.settled_count(cfg.min_matches, cfg.max_deviation);
        if settled == total && !state.finished {
            println!("All {total} pairs settled — answer more or press q to finish.");
        }

        let (mut a, mut b, kind) = pick(&state, &cfg, &mut rng);
        // Random presentation order kills position bias.
        if rng.random_bool(0.5) {
            std::mem::swap(&mut a, &mut b);
        }
        // Show both hands for each option, e.g. "QW | PO".
        let label = |i: usize| {
            let item = &state.items[i];
            format!("{}({})", item.label(), item.label_right())
        };
        let (label_a, label_b) = (label(a), label(b));

        // Re-prompt the same question until valid input; invalid lines are ignored.
        let score = loop {
            print!(
                "[{settled}/{total} settled, {} answered]  (1) {label_a}   (2) {label_b}  > ",
                state.history.len(),
            );
            std::io::stdout().flush().ok();

            let Some(Ok(line)) = lines.next() else {
                break None;
            };
            // React to the last typed character — stray input before it is ignored.
            match line.trim().chars().last() {
                Some('1') => break Some(1.0),
                Some('2') => break Some(0.0),
                Some('=') => break Some(0.5),
                Some('u') => {
                    let msg = if state.undo() {
                        "Undone."
                    } else {
                        "Nothing to undo."
                    };
                    println!("{msg}");
                    state.save(&session)?;
                }
                Some('s') => print_stats(&state, &cfg),
                Some('q') => break None,
                Some('?') => println!("? 1, 2, =, u, s or q"),
                _ => continue,
            }
        };
        let Some(score) = score else { break };

        if kind == PickKind::Audit {
            if contradicts(&state, a, b, score) {
                println!("Contradiction with earlier answers — both pairs re-opened.");
                state.reopen(a, b);
                state.finished = false;
                contradicted += 1;
            } else {
                confirmed += 1;
            }
        }
        state.answer(a, b, score);
        state.save(&session)?;
    }

    // A run that ends with everything settled marks the ranking as finished;
    // raw results are kept so the next run verifies it.
    if state.settled_count(cfg.min_matches, cfg.max_deviation) == state.items.len() {
        state.finished = true;
    }
    state.save(&session)?;
    if confirmed + contradicted > 0 {
        println!("Verification: {confirmed} confirmed, {contradicted} contradicted.");
    }
    write_outputs(&cfg, &state, &keyboard)?;
    Ok(())
}

/// Write ranked keyboard JSON and CSV report from current ratings.
fn write_outputs(cfg: &RankConfig, state: &RankState, keyboard: &Keyboard) -> Result<()> {
    let buckets = bucketize(state, cfg);
    let json = cfg.output_path();
    let csv = cfg.report_path();
    write_keyboard_json(&json, state, &buckets, keyboard)?;
    write_report_csv(&csv, state, &buckets, keyboard)?;
    println!("Wrote {} and {}", json.display(), csv.display());
    Ok(())
}

/// Print progress summary: best/worst pairs and confidence.
fn print_stats(state: &RankState, cfg: &RankConfig) {
    let mut order: Vec<&Item> = state.items.iter().collect();
    order.sort_by(|x, y| y.rating.total_cmp(&x.rating));
    let show = |items: &[&Item]| {
        items
            .iter()
            .map(|i| format!("{} ({:.0}±{:.0})", i.label(), i.rating, i.deviation))
            .collect::<Vec<_>>()
            .join("  ")
    };
    println!("best:  {}", show(&order[..5.min(order.len())]));
    println!("worst: {}", show(&order[order.len().saturating_sub(5)..]));
    println!(
        "settled {}/{}, answers {}",
        state.settled_count(cfg.min_matches, cfg.max_deviation),
        state.items.len(),
        state.history.len(),
    );
}
