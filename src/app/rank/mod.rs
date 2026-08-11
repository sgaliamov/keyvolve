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
            "{YELLOW}Saved ranking needs more confidence under the current model - resuming ranking.{RESET}"
        );
    }
    let mut rng = match cfg.seed {
        Some(seed) => StdRng::seed_from_u64(seed),
        None => StdRng::from_rng(&mut rand::rng()),
    };

    println!("{BOLD}Rank mode:{RESET} type the pair on your QWERTY keyboard, pick the EASIER one.");
    println!(
        "{DIM}Answers: ending letter / 1 / 2 = winner, = tie, ! suffix = lock (record multiple times); Shift for commands: N skip, U undo, S stats, C clear, Q quit (state is saved).{RESET}"
    );
    if state.finished {
        println!(
            "{CYAN}Ranking finished earlier - verification mode: checking saved ranking.{RESET}"
        );
    }

    // Verification counters for this run.
    let (mut confirmed, mut contradicted) = (0u32, 0u32);
    // In-run answer metadata so undo can replay the exact previous comparison
    // instead of jumping to a fresh random pick.
    let mut answered = Vec::<(usize, usize, PickKind)>::new();
    let mut repick = initial_forced_pick(&cfg, &state)?;

    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();

    while !app.should_finish() {
        let total = state.items.len();
        let settled = state.settled_count(&cfg);
        if settled == total && !state.finished {
            state.finished = true;
            state.save(&session)?;
            println!("{GREEN}All {total} pairs settled - entering verification mode.{RESET}");
        }

        let (mut a, mut b, kind) = if let Some(picked) = repick.take() {
            picked
        } else {
            let Some(picked) = pick(&state, &cfg, &mut rng) else {
                return Err(miette::miette!(
                    "No valid shared-key comparison is available for the current rank state"
                ));
            };
            picked
        };
        // Save cycle before answering (for uphill edges).
        let old_cycle = if kind == PickKind::Uphill {
            find_cycle(&state, a, b)
        } else {
            None
        };
        // Uphill questions: show the cycle this edge feeds, with per-hop
        // rating gaps — negative hop = the uphill jump being re-checked.
        if kind == PickKind::Uphill
            && let Some(ref cycle) = old_cycle
        {
            println!(
                "{DIM}[cycle]{RESET} {YELLOW}{path} {}{RESET}",
                state.items[cycle[cycle.len() - 1]].label(),
                path = format_cycle_path(&state, cycle),
            );
        }
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
            Score(f64, bool), // score, is_forced
            Skip,
            Repick,
            Quit,
        }
        let reply = loop {
            // Non-explore questions carry a marker: ⚡ uphill re-check (past
            // answer disagrees with the fit), ⚙ audit (consistency check).
            let mark = match kind {
                PickKind::Uphill => " [uphill]",
                PickKind::Audit => " [audit]",
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
            // Check for forced answer marker (! suffix).
            let trimmed = line.trim();
            let is_forced = trimmed.ends_with('!');
            let trimmed = if is_forced {
                &trimmed[..trimmed.len().saturating_sub(1)]
            } else {
                trimmed
            };
            // React to the last typed character — stray input before it is ignored.
            // Lowercase ending letters answer directly; commands are uppercase
            // (Shift) so they never clash with answers. 1/2/= still work.
            match trimmed.chars().last() {
                Some('1') => break Reply::Score(1.0, is_forced),
                Some('2') => break Reply::Score(0.0, is_forced),
                Some('=') => break Reply::Score(0.5, is_forced),
                Some(ch) if ch == last_a => break Reply::Score(1.0, is_forced),
                Some(ch) if ch == last_b => break Reply::Score(0.0, is_forced),
                Some('N') => break Reply::Skip,
                Some('U') => {
                    if let Some(ans) = state.undo() {
                        if state.settled_count(&cfg) < state.items.len() {
                            state.finished = false;
                        }
                        let kind = answered
                            .pop()
                            .map(|(_, _, kind)| kind)
                            .unwrap_or(PickKind::Explore);
                        repick = Some((ans.a, ans.b, kind));
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
                    "? ending letter / 1 / 2 = winner, = tie, ! = forced, N skip, U undo, S stats, C clear, Q quit"
                ),
                _ => continue,
            }
        };
        let (score, is_forced) = match reply {
            Reply::Score(score, forced) => (score, forced),
            Reply::Skip => continue,
            Reply::Repick => continue,
            Reply::Quit => break,
        };

        let checking = kind != PickKind::Explore;
        let contradiction = checking && contradicts(&state, a, b, score);
        if checking {
            if contradiction {
                println!("{RED}Contradiction with earlier answers - both pairs re-opened.{RESET}");
                state.finished = false;
                contradicted += 1;
            } else {
                confirmed += 1;
            }
        }
        // Record the answer; repeat if forced.
        let repeat_count = if is_forced {
            cfg.forced_answer_weight
        } else {
            1
        };
        for _ in 0..repeat_count {
            state.answer(a, b, score)?;
        }
        answered.push((a, b, kind));
        if is_forced {
            println!("{GREEN}! Locked (recorded {repeat_count}x){RESET}");
        }
        // Capture post-answer cycle text now, print it after the prompt line rewrite
        // so LINE_UP does not erase it.
        let cycle_after = old_cycle.map(|old_cycle| {
            if let Some(new_cycle) = find_cycle(&state, a, b) {
                let mut line = format!(
                    "{DIM}→ majority cycle still exists:{RESET} {CYAN}{path} {}{RESET}",
                    state.items[new_cycle[new_cycle.len() - 1]].label(),
                    path = format_cycle_path(&state, &new_cycle),
                );
                if !cycle_has_actionable_edge(&state, &new_cycle, &cfg) {
                    line.push_str(&format!(" {DIM}(no actionable uphill edge left){RESET}"));
                }
                line
            } else {
                format!(
                    "{GREEN}✓ Cycle resolved{RESET} {DIM}order:{RESET} {CYAN}{order}{RESET}",
                    order = format_cycle_order(&state, &old_cycle),
                )
            }
        });
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
        if let Some(cycle_after) = cycle_after {
            println!("{cycle_after}");
        }
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

fn initial_forced_pick(
    cfg: &RankConfig,
    state: &RankState,
) -> Result<Option<(usize, usize, PickKind)>> {
    let Some(spec) = cfg.force_check_pair.as_deref() else {
        return Ok(None);
    };
    let (a, b) = state.resolve_forced_check_pair(spec)?;
    println!("{CYAN}Forcing first check: {spec}{RESET}");
    Ok(Some((a, b, PickKind::Explore)))
}

/// Write ranked keyboard JSON and CSV report from current ratings.
fn write_outputs(cfg: &RankConfig, state: &RankState) -> Result<()> {
    let tiers = tierize(state, cfg);
    let json = cfg.output_path();
    let csv = cfg.report_path();
    let bigrams = cfg.bigrams_path();
    write_keyboard_json(&json, state, &tiers)?;
    write_report_csv(&csv, state, &tiers)?;
    write_bigrams_csv(&bigrams, state, &tiers)?;
    println!(
        "Wrote {}, {}, and {}",
        json.display(),
        csv.display(),
        bigrams.display()
    );
    Ok(())
}

/// Render a cycle with current ratings and per-hop fitted gaps.
fn format_cycle_path(state: &RankState, cycle: &[usize]) -> String {
    cycle
        .windows(2)
        .map(|e| {
            format!(
                "{}({:.0}) >{:+.0}>",
                state.items[e[0]].label(),
                state.items[e[0]].rating,
                state.items[e[0]].rating - state.items[e[1]].rating,
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Render the cycle nodes in current rating order after the cycle resolves.
fn format_cycle_order(state: &RankState, cycle: &[usize]) -> String {
    let mut nodes = cycle[..cycle.len().saturating_sub(1)].to_vec();
    nodes.sort_by(|a, b| state.items[*b].rating.total_cmp(&state.items[*a].rating));
    nodes
        .into_iter()
        .map(|i| format!("{}({:.0})", state.items[i].label(), state.items[i].rating))
        .collect::<Vec<_>>()
        .join(" > ")
}

/// True when any edge in a cycle still qualifies as an uphill re-check.
fn cycle_has_actionable_edge(state: &RankState, cycle: &[usize], cfg: &RankConfig) -> bool {
    cycle.windows(2).any(|e| {
        let winner = e[0];
        let loser = e[1];
        let mut wins = 0.0;
        let mut count = 0.0;
        for ans in &state.history {
            let (lo, hi) = (ans.a.min(ans.b), ans.a.max(ans.b));
            if lo == winner.min(loser) && hi == winner.max(loser) {
                count += 1.0;
                wins += if ans.a == winner {
                    ans.score
                } else {
                    1.0 - ans.score
                };
            }
        }
        let margin = wins - count / 2.0;
        let uphill = state.items[loser].rating - state.items[winner].rating;
        uphill > cfg.uphill_gap && margin.abs() <= cfg.thin_margin
    })
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
    let tiers = state.confidence_tier_count();
    println!(
        "{DIM}fit: log-loss{RESET} {:.3}{DIM}, agreement{RESET} {:.0}%{DIM}, spread/dev{RESET} {:.1}{DIM}, tiers{RESET} {}",
        loss / state.history.len() as f64,
        100.0 * hits as f64 / decisive.max(1) as f64,
        (max - min) / mean_dev,
        tiers,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_forced_pick_is_none_without_config() {
        let cfg = RankConfig::default();
        let state = RankState::new();
        assert!(initial_forced_pick(&cfg, &state).unwrap().is_none());
    }

    #[test]
    fn initial_forced_pick_resolves_requested_pair() {
        let cfg = RankConfig {
            force_check_pair: Some("AF-VE".to_owned()),
            ..Default::default()
        };
        let state = RankState::new();
        let (a, b, kind) = initial_forced_pick(&cfg, &state).unwrap().unwrap();
        assert_eq!(kind, PickKind::Explore);
        assert_eq!(state.items[a].label(), "AF");
        assert_eq!(state.items[b].label(), "VE");
    }
}
