use super::{Behavior, Efforts, FrozenKeys, Position};
use ed_balance::{CliSettings, Context};
use itertools::Itertools;
use serde_json::{self, Value};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

/// Constructs a `Behavior` from CLI settings, returning `None` on any
/// missing or malformed input so the caller can surface a clean error
/// instead of panicking deep inside the GA loop.
pub fn create(settings: &CliSettings) -> Option<Behavior> {
    let context = Context::new(settings);
    // The keyboard JSON file describes the physical layout (efforts, blocked
    // keys, frozen keys).  Without it there is nothing to optimise.
    let path = settings.keyboard.clone()?;
    let json = load_json(&path)?;
    // The text corpus is what the scorer uses to evaluate layouts; loading it
    // once here avoids repeated disk I/O during the inner GA loop.
    let words = load_words(&settings.text.clone()?)?;
    let frozen_keys = load_frozen(&json)?;
    let efforts = load_efforts(&json)?;
    // These two penalties are user-tunable so they live in the JSON config
    // rather than being hard-coded, making it easy to experiment.
    let switch_penalty = json["switchPenalty"].as_f64()?;
    let same_key_penalty = json["sameKeyPenalty"].as_f64()?;
    // `blocked` is an array of integer position indices in the JSON.
    let blocked_keys: HashSet<Position> = json["blocked"]
        .as_array()?
        .into_iter()
        .map(|x| x.as_u64().unwrap() as Position)
        .collect();

    Some(Behavior {
        context,
        words,
        frozen_keys,
        efforts,
        switch_penalty,
        same_key_penalty,
        blocked_keys,
    })
}

fn load_words(path: &PathBuf) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(path).ok()?;
    let words = text.split(' ').map_into().collect_vec();
    Some(words)
}

fn parse_u8(str: &String) -> Option<Position> {
    str.parse::<Position>().ok()
}

// Effort values in the JSON use a 1–5 scale so they are easy to author.
// Internally we rescale them to 1–`maxEffort` so the genetic algorithm
// can work with whatever absolute weight range the user prefers.
const MIN_VALUE: f64 = 1.;
const MAX_VALUE: f64 = 5.;

/// Linearly maps a raw JSON effort value from [1, 5] to [1, maxEffort].
/// Keeping 1 as the floor means a "free" key never contributes zero cost,
/// which avoids degenerate layouts that stack everything on one key.
fn normalize_effort(value: f64, factor: f64) -> f64 {
    debug_assert!(
        value >= MIN_VALUE,
        "Minimal allowed value is {}",
        MIN_VALUE
    );
    debug_assert!(
        value <= 5.,
        "Maximal allowed value is {}",
        MAX_VALUE
    );

    (value - 1.) * factor + 1.
}

fn parse_nested_efforts(
    json: &Value,
    keys_shift: Position,
    factor: f64,
) -> Option<HashMap<Position, f64>> {
    json.as_object()?
        .iter()
        .map(|(key, value)| {
            let key = parse_u8(key)? + keys_shift;
            let value = normalize_effort(value.as_f64()?, factor);
            Some((key, value))
        })
        .collect()
}

fn parse_efforts(json: &Value, keys_shift: Position, factor: f64) -> Option<Efforts> {
    json["efforts"]
        .as_object()?
        .iter()
        .map(|(key, value)| {
            let key = parse_u8(key)? + keys_shift;
            let value = parse_nested_efforts(value, keys_shift, factor)?;
            Some((key, value))
        })
        .collect()
}

fn get_factor(max: f64) -> f64 {
    (max - 1.) / (MAX_VALUE - 1.)
}

fn load_efforts(json: &Value) -> Option<Efforts> {
    let max = json["maxEffort"].as_f64()?;
    let factor = get_factor(max);
    let mut left = parse_efforts(json, 0, factor)?;
    // The right half of the keyboard mirrors the left ergonomically, so we
    // reuse the same effort table and simply offset all position indices by
    // 15 (the number of left-hand slots).  This halves the config authoring
    // burden; a non-symmetric board could provide separate right-hand efforts.
    let right = parse_efforts(json, 15, factor)?;
    left.extend(right);

    Some(left)
}

fn load_json(keyboard: &PathBuf) -> Option<Value> {
    let content = std::fs::read_to_string(keyboard).ok()?;
    serde_json::from_str(&content).ok()
}

fn load_frozen(json: &Value) -> Option<FrozenKeys> {
    json["frozen"]
        .as_object()?
        .iter()
        .map(|(key, value)| {
            let key = key.chars().next()?;
            let value = parse_u8(&value.as_str()?.to_string())?;
            Some((key, value))
        })
        .collect()
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_load() {
        let json = json!({
        "maxEffort": 5,
        "efforts": {
            "0": {
                "0": 1,
                "1": 2
            },
            "1": {
                "0": 3,
                "1": 4
            },
        }});
        let actual = load_efforts(&json).unwrap();
        let expected: Efforts = [
            (0, [(0, 1.), (1, 2.)].iter().cloned().collect()),
            (1, [(0, 3.), (1, 4.)].iter().cloned().collect()),
            (15, [(15, 1.), (16, 2.)].iter().cloned().collect()),
            (16, [(15, 3.), (16, 4.)].iter().cloned().collect()),
        ]
        .iter()
        .cloned()
        .collect();

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_normalize_effort_for_1() {
        let factor = get_factor(3.);
        let actual = normalize_effort(1., factor);

        assert_eq!(actual, 1.);
    }

    #[test]
    fn test_normalize_effort_for_2() {
        let factor = get_factor(2.);
        let actual = normalize_effort(3., factor);

        assert_eq!(actual, 1.5);
    }

    #[test]
    fn test_normalize_effort_for_3() {
        let factor = get_factor(3.);
        let actual = normalize_effort(3., factor);

        assert_eq!(actual, 2.);
    }

    #[test]
    fn test_normalize_effort_for_4() {
        let factor = get_factor(4.);
        let actual = normalize_effort(3., factor);

        assert_eq!(actual, 2.5);
    }

    #[test]
    fn test_normalize_effort_for_5() {
        let factor = get_factor(3.);
        let actual = normalize_effort(5., factor);

        assert_eq!(actual, 3.);
    }
}
