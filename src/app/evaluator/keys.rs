use crate::models::Keys;
use crate::models::slot_row;

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
