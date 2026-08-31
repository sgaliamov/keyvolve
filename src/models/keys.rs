use rustc_hash::FxHashMap;

/// char → physical slot index (0–29).
pub type Keys = FxHashMap<char, u8>;

/// Row index within a hand (0 = top, 2 = bottom).
#[inline]
pub fn slot_row(slot: u8) -> u8 {
    (slot % 15) / 5
}

/// Slot for `c`; panic names the offending char so corpus/layout mismatches are debuggable.
#[inline]
pub fn slot(keys: &Keys, c: char) -> u8 {
    *keys
        .get(&c)
        .unwrap_or_else(|| panic!("char {c:?} (U+{:04X}) not in layout: {keys:?}", c as u32))
}

/// Weighted same-hand row-switch cost. Adjacent-row move = 1, jump-over-row = 2.
#[inline]
pub fn row_distance(from: u8, to: u8) -> u64 {
    slot_row(from).abs_diff(slot_row(to)).into()
}
