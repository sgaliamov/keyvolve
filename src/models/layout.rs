use itertools::Itertools;
use miette::{IntoDiagnostic, Result, miette};
use rustc_hash::FxHashMap;
use std::{
    fmt,
    fs::File,
    io::{self, BufRead},
    path::Path,
};

pub type Keys = FxHashMap<char, u8>;

#[derive(Clone)]
pub struct Layout {
    pub keys: Keys,
    /// Display name: explicit from the input CSV `name` column, else home-row letters.
    pub name: String,
}

impl Layout {
    /// Build from a layout/CSV line. Name = the `name` column when present,
    /// otherwise the home-row letters.
    #[allow(dead_code)]
    pub fn new(line: &str) -> Self {
        Self::try_new(line).unwrap_or_else(|e| panic!("invalid layout row: {e}"))
    }

    /// Fallible layout parser. Rejects malformed rows.
    pub fn try_new(line: &str) -> Result<Self> {
        let keys = line_to_keys(line)?;
        let name = name_field(line)
            .map(str::to_string)
            .unwrap_or_else(|| home_row_name(&keys));
        Ok(Layout { keys, name })
    }

    /// Build from a 30-slot char array; name derived from the home row.
    pub fn from_keys(keys: &[char]) -> Self {
        let keys: Keys = keys
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_alphabetic())
            .map(|(i, &c)| (c, i as u8))
            .collect();
        let name = home_row_name(&keys);
        Layout { keys, name }
    }

    /// Hand-swapped twin: every key reflected left↔right. Involution.
    /// Fitness is hand-symmetric, so a layout and its mirror score identically.
    /// Name travels with the layout unchanged.
    pub fn mirrored(&self) -> Layout {
        let keys = self
            .keys
            .iter()
            .map(|(&c, &p)| (c, mirror_slot(p as usize) as u8))
            .collect();
        Layout {
            keys,
            name: self.name.clone(),
        }
    }

    /// `true` when `e` sits on the left hand (slot 0–14); `false` if on the right
    /// or absent. Drives canonicalization to the `e`-left orientation on save.
    pub fn e_is_left(&self) -> bool {
        self.keys.get(&'e').is_some_and(|&p| p < 15)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Vec<Layout>> {
        let path = path.as_ref();
        let file = File::open(path).into_diagnostic()?;

        let mut seen = rustc_hash::FxHashSet::default();
        let mut layouts = Vec::new();
        for line in io::BufReader::new(file).lines() {
            let line = line.into_diagnostic()?;
            let line = line.trim();
            if line.is_empty() || is_header(line) {
                continue;
            }
            if !seen.insert(line.splitn(7, ',').take(6).collect::<String>()) {
                continue;
            }
            layouts.push(Layout::try_new(line)?);
        }
        Ok(layouts)
    }

    /// 30-slot character array; `_` marks an empty slot. Index = physical key position.
    fn slots(&self) -> [char; 30] {
        let mut slots = ['_'; 30];
        for (&ch, &pos) in &self.keys {
            slots[pos as usize] = ch;
        }
        slots
    }
}

impl fmt::Display for Layout {
    /// Reconstruct comma-separated layout string (positions 0–14 left; 15–29 right, stored inner→outer per group).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let slots = self.slots();
        let left = slots[..15]
            .chunks(5)
            .map(|c| c.iter().collect::<String>())
            .join(",");
        let right = slots[15..]
            .chunks(5)
            .map(|c| c.iter().collect::<String>())
            .join(",");
        write!(f, "{left},{right}")
    }
}

/// Detect persisted CSV header row.
fn is_header(line: &str) -> bool {
    line.starts_with("keys_1,")
}

/// Hand-swap reflection of a slot index (0–29). Involution: `mirror_slot(mirror_slot(i)) == i`.
/// Left col k (slots 0–14) ↔ right col 4-k (slots 15–29), same row.
fn mirror_slot(i: usize) -> usize {
    if i < 15 {
        (i / 5) * 5 + (4 - i % 5) + 15
    } else {
        let r = i - 15;
        (r / 5) * 5 + (4 - r % 5)
    }
}

pub fn line_to_keys(line: &str) -> Result<Keys> {
    let groups = line.split(',').map(str::trim).collect_vec();
    if groups.len() < 6 {
        return Err(miette!(
            "layout row needs 6 key groups, got {}",
            groups.len()
        ));
    }

    let mut keys = Keys::default();
    for (group_idx, group) in groups.into_iter().take(6).enumerate() {
        let chars = group.chars().collect_vec();
        if chars.len() != 5 {
            return Err(miette!(
                "layout group {} must have 5 slots, got {}",
                group_idx + 1,
                chars.len()
            ));
        }

        for (slot_idx, c) in chars.into_iter().enumerate() {
            if c == '_' {
                continue;
            }
            if !c.is_ascii_alphabetic() {
                return Err(miette!("invalid layout key {c:?}"));
            }
            let pos = (group_idx * 5 + slot_idx) as u8;
            if keys.insert(c, pos).is_some() {
                return Err(miette!("duplicate layout key {c:?}"));
            }
        }
    }

    if keys.len() != 26 {
        let preview = line
            .split(',')
            .take(7)
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(" | ");
        return Err(miette!(
            "layout must contain all 26 keys, got {} | row: [{}]",
            keys.len(),
            preview
        ));
    }

    Ok(keys)
}

/// Physical home-row slots — left 5–9, right 20–24.
const HOME_ROW: [usize; 10] = [5, 6, 7, 8, 9, 20, 21, 22, 23, 24];

/// Explicit name from the CSV column after the six key groups. `None` when absent
/// or numeric — old headerless rows store fitness there, not a name.
pub fn name_field(line: &str) -> Option<&str> {
    line.split(',')
        .nth(6)
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.parse::<f64>().is_err())
}

/// Home-row letters (slots 5–9, 20–24), empties skipped — the auto-name fallback.
fn home_row_name(keys: &Keys) -> String {
    let mut slots = ['_'; 30];
    for (&c, &p) in keys {
        slots[p as usize] = c;
    }
    HOME_ROW
        .into_iter()
        .map(|i| slots[i])
        .filter(|c| c.is_alphabetic())
        .collect()
}

#[cfg(test)]
mod layout_test {
    use super::*;

    #[test]
    fn test_line_to_keys_basic() {
        let line = "zydpx, ralem, vbjuq, whtc_, fnosi, kg___, not used tail";
        let keys = line_to_keys(line).unwrap();

        assert_eq!(keys.len(), 26);
        assert_eq!(keys[&'z'], 0);
        assert_eq!(keys[&'x'], 4);
        assert_eq!(keys[&'q'], 14);
        assert_eq!(keys[&'w'], 15);
        assert_eq!(keys[&'c'], 18);
        assert_eq!(keys[&'g'], 26);
    }

    #[test]
    fn test_name() {
        let line = "zydpx,ralem,vbjuq,whtc_,fnosi,kg___,not used tail";
        let layout = Layout::try_new(line).unwrap();

        assert_eq!(layout.to_string(), "zydpx,ralem,vbjuq,whtc_,fnosi,kg___");
    }

    #[test]
    fn right_block_anchors_slot_15_to_29() {
        // Right hand starts at slot 15 (top-left, inner) and ends at slot 29 (bottom-right, outer).
        let line = "abcde, fghij, klmno, pqrst, uvwxy, ____z";
        let keys = line_to_keys(line).unwrap();

        assert_eq!(keys[&'a'], 0); // left top-left
        assert_eq!(keys[&'o'], 14); // left bottom-right
        assert_eq!(keys[&'p'], 15); // right top-left (start)
        assert_eq!(keys[&'t'], 19); // right top-right — locks inner→outer direction
        assert_eq!(keys[&'z'], 29); // right bottom-right (end)
    }

    #[test]
    fn display_round_trips_filled_bottom_right() {
        // Letter on slot 29 survives render at the bottom-right.
        let line = "abcde,fghij,klmno,pqrst,uvwxy,____z";

        assert_eq!(Layout::try_new(line).unwrap().to_string(), line);
    }

    #[test]
    fn mirrored_is_an_involution() {
        let layout = Layout::try_new("zydpx, ralem, vbjuq, whtc_, fnosi, kg___").unwrap();

        assert_eq!(layout.mirrored().mirrored().to_string(), layout.to_string());
    }

    #[test]
    fn mirrored_swaps_e_hand() {
        // `e` at slot 8 (left); mirroring moves it to the right hand.
        let layout = Layout::try_new("zydpx, ralem, vbjuq, whtc_, fnosi, kg___").unwrap();

        assert!(layout.e_is_left());
        assert!(!layout.mirrored().e_is_left());
    }

    #[test]
    fn new_derives_home_row_name_when_absent() {
        let layout = Layout::try_new("abcde, fghij, klmno, pqrst, uvwxy, z____").unwrap();

        assert_eq!(layout.name, "fghijuvwxy");
    }

    #[test]
    fn new_uses_explicit_name_column() {
        let layout =
            Layout::try_new("abcde, fghij, klmno, pqrst, uvwxy, z____, dvorak, 12.5").unwrap();

        assert_eq!(layout.name, "dvorak");
    }

    #[test]
    fn from_keys_derives_home_row_name() {
        let slots: Vec<char> = "abcdefghijklmnopqrstuvwxy_____".chars().collect();

        assert_eq!(Layout::from_keys(&slots).name, "fghijuvwxy");
    }

    #[test]
    fn mirrored_keeps_name() {
        let layout =
            Layout::try_new("abcde, fghij, klmno, pqrst, uvwxy, z____, dvorak, 12.5").unwrap();

        assert_eq!(layout.mirrored().name, "dvorak");
    }

    #[test]
    fn rejects_missing_group() {
        assert!(Layout::try_new("abcde,fghij,klmno,pqrst,uvwxy").is_err());
    }

    #[test]
    fn rejects_duplicate_key() {
        assert!(Layout::try_new("abcde,fghij,klmno,pqrst,uvwxa,_____").is_err());
    }

    #[test]
    fn rejects_bad_character() {
        assert!(Layout::try_new("abcde,fghij,klmno,pqrst,uvwxy,____!").is_err());
    }
}
