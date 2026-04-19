use super::{Behavior, Position};
use crate::keyboard::{get_factor, Keys, Score};
use itertools::Itertools;
use std::collections::HashMap;

/// Aggregates per-word scores across the entire corpus into a single `Score`
/// tuple.  Lower total effort is better because it means fewer finger
/// movements and better hand balance.  We accumulate left/right counters and
/// effort sums separately so the balance factor can be applied once at the
/// end rather than inside the tight per-word loop.
pub fn calculate_score(this: &Behavior, keyboard: &Keys) -> Score {
    let (effort, left_counter, right_counter, switch, left_effort, right_effort) = this
        .words
        .iter()
        .map(|x| calculate_word_score(this, keyboard, x))
        .fold(
            (0., 0, 0, 0, 0., 0.),
            |(
                effort_total,
                left_total,
                right_total,
                switch_total,
                left_effort_total,
                right_effort_total,
            ),
             (
                word_effort,
                word_left,
                word_right,
                word_switch,
                word_left_effort,
                word_right_effort,
            )| {
                (
                    effort_total + word_effort,
                    left_total + word_left,
                    right_total + word_right,
                    switch_total + word_switch,
                    left_effort_total + word_left_effort,
                    right_effort_total + word_right_effort,
                )
            },
        );

    // Apply the balance factor after accumulation so that a layout with equal
    // left/right effort gets factor ≈ 1 (no penalty) while an unbalanced one
    // gets factor > 1, effectively raising its score and pushing it down the
    // ranking.
    let factor = get_factor(left_effort, right_effort);
    let effort = effort * factor;

    (
        effort,
        left_counter,
        right_counter,
        switch,
        left_effort,
        right_effort,
    )
}

/// Scores a single word by iterating over consecutive character pairs
/// (bigrams).  Effort is looked up from the pre-computed efforts table and
/// adjusted by penalties for same-key repetition or hand switches.
fn calculate_word_score(
    Behavior: &Behavior,
    keyboard: &HashMap<char, Position>,
    word: &str,
) -> Score {
    #[inline]
    fn is_left(position: Position) -> bool {
        // Positions 0-14 are the left half; 15-29 are the right half.
        position < 15
    }

    let chars = word.chars().collect_vec();
    let key = keyboard[&chars[0]];
    // The first character of a word has no predecessor, so we use the
    // self-effort (key pressed in isolation) as its contribution.  This
    // ensures single-character words still accumulate a non-zero score.
    let first = Behavior.efforts[&key][&key];
    let (score, left, right, switch, left_effort, right_effort) = chars
        .iter()
        .tuple_windows()
        .map(|(a, b)| {
            let key_a = keyboard[a];
            let key_b = keyboard[b];
            let a_is_left = is_left(key_a);
            let b_is_left = is_left(key_b);
            let both_left = a_is_left && b_is_left;
            let both_right = !a_is_left && !b_is_left;
            let switch = a_is_left != b_is_left;

            if switch {
                // When hands alternate, key `a` was already counted in the
                // previous iteration.  We charge the self-effort of key `b`
                // here because the new hand is starting a fresh sequence
                // (analogous to the first-letter cost above), multiplied by
                // the switch penalty to discourage frequent alternation on
                // high-cost positions.
                let effort = Behavior.efforts[&key_b][&key_b];

                return (
                    Behavior.switch_penalty * effort,
                    both_left as u32,
                    both_right as u32,
                    switch as u32,
                    if both_left { effort } else { 0. },
                    if both_right { effort } else { 0. },
                );
            }

            let effort = Behavior.efforts[&key_a][&key_b];

            if key_a == key_b {
                return (
                    effort * Behavior.same_key_penalty,
                    both_left as u32,
                    both_right as u32,
                    switch as u32,
                    if both_left { effort } else { 0. },
                    if both_right { effort } else { 0. },
                );
            }

            (
                effort,
                both_left as u32,
                both_right as u32,
                switch as u32,
                if both_left { effort } else { 0. },
                if both_right { effort } else { 0. },
            )
        })
        .fold(
            (0., 0, 0, 0, 0., 0.),
            |(total, left, right, total_switch, total_left_effort, total_right_effort),
             (effort, both_left, both_right, switch, left_effort, right_effort)| {
                (
                    effort + total,
                    left + both_left,
                    right + both_right,
                    total_switch + switch,
                    total_left_effort + left_effort,
                    total_right_effort + right_effort,
                )
            },
        );

    (
        score + first,
        left,
        right,
        switch,
        left_effort,
        right_effort,
    )
}
