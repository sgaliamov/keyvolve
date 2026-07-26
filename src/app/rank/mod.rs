pub mod config;
mod fit;
mod output;
mod select;
mod state;

use cliffa::cli::AppHandle;
pub use config::*;
use miette::Result;
pub use output::*;
use rand::{RngExt, SeedableRng, rngs::StdRng};
pub use select::*;
pub use state::*;
use std::io::{BufRead, Write};

// ANSI colors for interactive messages.
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";
const LINE_UP: &str = "\x1b[1A\x1b[2K";
const CLEAR: &str = "\x1b[2J\x1b[H";

/// Interactive pair-ranking mode: repeatedly asks the user which of two
/// bigram pairs is easier to type, fitting Bradley–Terry ratings for all 210
/// ordered left-hand pairs. Resumable; writes ranked keyboard JSON + CSV report.
pub fn rank(cfg: RankConfig, app: AppHandle) -> Result<()> {
    cfg.validate()?;
    let session = cfg.session_path();
    let mut state = RankState::load_or_new(&session)?;
    if state.finished && state.settled_count(&cfg) < state.items.len() {
        state.finished = false;
        println!(
            "{YELLOW}Saved ranking needs more confidence under the current model — resuming ranking.{RESET}"
        );
    }
    let mut rng = match cfg.seed {
        Some(seed) => StdRng::seed_from_u64(seed),
        None => StdRng::from_rng(&mut rand::rng()),
    };

    println!("{BOLD}Rank mode:{RESET} type the pair on your QWERTY keyboard, pick the EASIER one.");
    println!(
        "{DIM}Answers: ending letter / 1 / 2 = winner, = tie; Shift for commands: N skip, U undo, S stats, C clear, Q quit (state is saved).{RESET}"
    );
    if state.finished {
        println!(
            "{CYAN}Ranking finished earlier — verification mode: checking saved ranking.{RESET}"
        );
    }

    // Verification counters for this run.
    let (mut confirmed, mut contradicted) = (0u32, 0u32);

    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();

    while !app.should_finish() {
        let total = state.items.len();
        let settled = state.settled_count(&cfg);
        if settled == total && !state.finished {
            state.finished = true;
            state.save(&session)?;
            println!("{GREEN}All {total} pairs settled — entering verification mode.{RESET}");
        }

        let (mut a, mut b, kind) = {
            let Some(picked) = pick(&state, &cfg, &mut rng) else {
                return Err(miette::miette!(
                    "No valid shared-key comparison is available for the current rank state"
                ));
            };
            picked
        };
        // Random presentation order kills position bias.
        if rng.random_bool(0.5) {
            std::mem::swap(&mut a, &mut b);
        }
        // Show both hands for each option, e.g. "QW | PO".
        let label = |i: usize| {
            let item = &state.items[i];
            format!("{}[{}]", item.label(), item.label_right())
        };
        let (label_a, label_b) = (label(a), label(b));
        // Options normally start from the same key, so the ending letter
        // identifies the winner (e.g. TE vs TD → 'e' or 'd'). Uphill re-checks
        // of historical pairs may share the ENDING key instead (e.g. WD vs
        // RD) — then the starting letter distinguishes (w/r).
        let letter = |slot: u8| QWERTY[slot as usize].to_ascii_lowercase();
        let key = |i: usize| {
            let item = &state.items[i];
            (item.from, item.to)
        };
        let ((from_a, to_a), (from_b, to_b)) = (key(a), key(b));
        let (last_a, last_b) = if to_a != to_b {
            (letter(to_a), letter(to_b))
        } else {
            println!(
                "{CYAN}Both options end with '{}' — answer with the STARTING letter:{RESET}",
                letter(to_a)
            );
            (letter(from_a), letter(from_b))
        };

        // Re-prompt the same question until valid input; invalid lines are ignored.
        // `Skip` moves on without recording an answer.
        enum Reply {
            Score(f64),
            Skip,
            Repick,
            Quit,
        }
        let reply = loop {
            // Non-explore questions carry a marker: ⚡ uphill re-check (past
            // answer disagrees with the fit), ⚙ audit (consistency check).
            let mark = match kind {
                PickKind::Uphill => " ⚡",
                PickKind::Audit => " ⚙",
                PickKind::Explore => "",
            };
            print!(
                "{DIM}[{settled}/{total} settled, {} answered]{RESET}{YELLOW}{mark}{RESET}  1: {BOLD}{label_a}{RESET}   2: {BOLD}{label_b}{RESET}  > ",
                state.history.len(),
            );
            std::io::stdout().flush().ok();

            let Some(Ok(line)) = lines.next() else {
                break Reply::Quit;
            };
            // React to the last typed character — stray input before it is ignored.
            // Lowercase ending letters answer directly; commands are uppercase
            // (Shift) so they never clash with answers. 1/2/= still work.
            match line.trim().chars().last() {
                Some('1') => break Reply::Score(1.0),
                Some('2') => break Reply::Score(0.0),
                Some('=') => break Reply::Score(0.5),
                Some(ch) if ch == last_a => break Reply::Score(1.0),
                Some(ch) if ch == last_b => break Reply::Score(0.0),
                Some('N') => break Reply::Skip,
                Some('U') => {
                    if state.undo() {
                        if state.settled_count(&cfg) < state.items.len() {
                            state.finished = false;
                        }
                        println!("{YELLOW}Undone.{RESET}");
                        state.save(&session)?;
                        break Reply::Repick;
                    }
                    println!("Nothing to undo.");
                }
                Some('S') => {
                    print_stats(&state, &cfg);
                    write_outputs(&cfg, &state)?;
                }
                Some('C') => print!("{CLEAR}"),
                Some('Q') => break Reply::Quit,
                Some('?') => println!(
                    "? ending letter / 1 / 2 = winner, = tie, N skip, U undo, S stats, C clear, Q quit"
                ),
                _ => continue,
            }
        };
        let score = match reply {
            Reply::Score(score) => score,
            Reply::Skip => continue,
            Reply::Repick => continue,
            Reply::Quit => break,
        };

        let checking = kind != PickKind::Explore;
        let contradiction = checking && contradicts(&state, a, b, score);
        if checking {
            if contradiction {
                println!("{RED}Contradiction with earlier answers — both pairs re-opened.{RESET}");
                state.finished = false;
                contradicted += 1;
            } else {
                confirmed += 1;
            }
        }
        state.answer(a, b, score)?;
        // Rewrite the prompt line in place: erase user's input, append the
        // picked winner plus updated model stats for both options. Arrows show
        // rating/deviation direction; fixed column widths keep lines aligned.
        let last = state.history.last().expect("answer just recorded");
        let prev = |i: usize| {
            if i == last.a {
                last.prev_a
            } else {
                last.prev_b
            }
        };
        // Direction arrow for a metric vs its pre-answer snapshot.
        let dir = |now: f64, before: f64| match now.total_cmp(&before) {
            std::cmp::Ordering::Greater => "↑",
            std::cmp::Ordering::Less => "↓",
            std::cmp::Ordering::Equal => "·",
        };
        let stat = |i: usize| {
            let it = &state.items[i];
            let (prev_rating, prev_deviation, _) = prev(i);
            // Winner's label is highlighted instead of a separate pick column.
            let won = (i == a && score > 0.5) || (i == b && score < 0.5);
            let label = if won {
                format!("{RESET}{BOLD}{}{RESET}{DIM}", it.label())
            } else {
                it.label()
            };
            format!(
                "{label} {:04.0}{}±{:03.0}{} m:{:02}",
                it.rating,
                dir(it.rating, prev_rating),
                it.deviation,
                dir(it.deviation, prev_deviation),
                it.matches
            )
        };
        // Rating gap between the two options: growing gap = cleaner separation,
        // shrinking gap = this answer contradicted the model.
        let gap = (state.items[a].rating - state.items[b].rating).abs();
        let prev_gap = (last.prev_a.0 - last.prev_b.0).abs();
        println!(
            "{LINE_UP}{DIM}[{settled}/{total}] {}  {}  gap:{:03.0}{}{RESET}",
            stat(a),
            stat(b),
            gap,
            dir(gap, prev_gap),
        );
        if contradiction {
            state.reopen(a, b);
        }
        state.save(&session)?;
    }

    // A run that ends with everything settled marks the ranking as finished;
    // raw results are kept so the next run verifies it.
    if state.settled_count(&cfg) == state.items.len() {
        state.finished = true;
    }
    state.save(&session)?;
    print_stats(&state, &cfg);
    if confirmed + contradicted > 0 {
        println!(
            "{DIM}Verification:{RESET} {CYAN}{confirmed} confirmed, {contradicted} contradicted.{RESET}"
        );
    }
    write_outputs(&cfg, &state)?;
    Ok(())
}

/// Write ranked keyboard JSON and CSV report from current ratings.
fn write_outputs(cfg: &RankConfig, state: &RankState) -> Result<()> {
    let buckets = bucketize(state, cfg);
    let json = cfg.output_path();
    let csv = cfg.report_path();
    write_keyboard_json(&json, state, &buckets)?;
    write_report_csv(&csv, state, &buckets)?;
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
    println!("{DIM}best: {RESET} {}", show(&order[..10.min(order.len())]));
    println!(
        "{DIM}worst:{RESET} {}",
        show(&order[order.len().saturating_sub(10)..])
    );
    println!(
        "{DIM}settled{RESET} {}/{}{DIM}, answers{RESET} {}{DIM}, roughly ~{} answers left{RESET}",
        state.settled_count(cfg),
        state.items.len(),
        state.history.len(),
        state.steps_left(cfg),
    );
    print_fit_quality(state);
}

/// Global fit quality: how well current ratings explain the recorded answers.
/// See docs/rank-mode.md "Reading the fit quality line" for interpretation.
fn print_fit_quality(state: &RankState) {
    if state.history.is_empty() {
        return;
    }
    let (loss, hits, decisive) = state.history.iter().fold((0.0, 0, 0), |(l, h, d), a| {
        let p = fit::expected_score(state.items[a.a].rating, state.items[a.b].rating);
        let l = l - (a.score * p.ln() + (1.0 - a.score) * (1.0 - p).ln());
        match a.score {
            1.0 => (l, h + usize::from(p > 0.5), d + 1),
            0.0 => (l, h + usize::from(p < 0.5), d + 1),
            _ => (l, h, d),
        }
    });
    let (min, max) = state
        .items
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), i| {
            (lo.min(i.rating), hi.max(i.rating))
        });
    let mean_dev = state.items.iter().map(|i| i.deviation).sum::<f64>() / state.items.len() as f64;
    // Effective tiers: how many statistically distinct levels the whole
    // spread can hold at 95% confidence.
    let tiers = (max - min) / (1.96 * mean_dev);
    println!(
        "{DIM}fit: log-loss{RESET} {:.3}{DIM}, agreement{RESET} {:.0}%{DIM}, spread/dev{RESET} {:.1}{DIM}, tiers{RESET} {:.0}",
        loss / state.history.len() as f64,
        100.0 * hits as f64 / decisive.max(1) as f64,
        (max - min) / mean_dev,
        tiers,
    );
}
