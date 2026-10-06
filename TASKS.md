# Tasks and Bugs

## Tasks

- [x] Forbid gaps inside row segments: empties only at segment ends (`g_jv_` invalid); reject configs where a gap is unavoidable - [doc](docs/placement-rules.md#row-gaps).
- [ ] Rank flat CSV: drop the report stem from the name; write `bigrams.<ext>` in the `report` folder instead of `<report stem>.bigrams.<ext>` - [doc](docs/modes/rank.md#output).
- [ ] Stats mode should write separate CSV reports for first-letter and aggregated bigram distributions so people can inspect them without JSON - [doc](docs/modes/stats.md#output).
- [x] Remove `stats.minFrequency`; all bigrams and first letters must be counted, without threshold filtering - [doc](docs/modes/stats.md#configuration).

## Bugs

- [x] Merge drops single-letter words; preserve them so letter and first-letter stats include them — [doc](docs/modes/merge.md#cleaning-rules).
- [ ] Merge skips subfolders; scan the input folder recursively for matching files — [doc](docs/modes/merge.md#input-selection).
- [ ] Merge extension filter is case-sensitive; accept `.txt` case-insensitively — [doc](docs/modes/merge.md#input-selection).
- [ ] Stats `average_word_length` counts UTF-8 bytes; count characters - [doc](docs/modes/stats.md#what-is-counted).
- [ ] Stats ignores Ctrl+C; stop reading and leave `output` untouched - [doc](docs/modes/stats.md#interruption).
- [ ] `allowed._` accepts slots 15-29; reject them like letter keys (0-14 only, mirrored) - [doc](docs/placement-rules.md#configuration).
- [ ] `allowed` slots ≥30 report `must be in 0..29`; any slot outside 0-14 must report `allowed slot N must be in 0..14` for every key, `_` included - [doc](docs/placement-rules.md#startup-checks).
- [ ] `signed_imbalance_percent` is not mirror-symmetric (`L = 2R` → +100, `R = 2L` → -50); swapping hands must flip only the sign, so mirror twins score identically - [doc](docs/evaluation/penalty.md#metric-catalog).
- [ ] Evaluate with `output` set overwrites `output` with partial results on Ctrl+C; an interrupted run must write nothing - [doc](docs/evaluation/evaluate.md#interruption).
- [ ] Review the console output format in `evaluate` and ensure each printed row is readable and the penalty breakdown is still useful - [doc](docs/evaluation/evaluate.md#console).
