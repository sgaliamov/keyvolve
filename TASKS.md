# Tasks and Bugs

## Tasks

- [ ] Do not allow `g_jv_`.

## Bugs

- [ ] Merge drops single-letter words; preserve them so letter and first-letter stats include them — [doc](docs/modes/merge.md#cleaning-rules).
- [ ] Merge skips subfolders; scan the input folder recursively for matching files — [doc](docs/modes/merge.md#input-selection).
- [ ] Merge extension filter is case-sensitive; accept `.txt` case-insensitively — [doc](docs/modes/merge.md#input-selection).
- [ ] Stats `average_word_length` counts UTF-8 bytes; count characters - [doc](docs/modes/stats.md#what-is-counted).
- [ ] Stats ignores Ctrl+C; stop reading and leave `output` untouched - [doc](docs/modes/stats.md#interruption).
