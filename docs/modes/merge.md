# Merge mode — how it works

Merge mode turns a folder of raw `.txt` files into one cleaned corpus file: lowercase ASCII
words, one per line, optionally shuffled. The result is the source text for `stats`
(`stats.input`) and `synthesise` (`synthesise.text`).

## Running

```sh
keyvolve -m merge
```

For configuration overrides and precedence, see the [CLI documentation](../../cliffa/README.md).

## Configuration

```yaml
merge:
  input: data/samples
  output: data/samples/merged.txt
  shuffle: true
  # seed: 12345
```

| Key | Default | Meaning |
| --- | --- | --- |
| `input` | — (required) | Folder with source `.txt` files. |
| `output` | — (required) | Merged corpus file. Overwritten if it exists; missing parent folders are created. |
| `shuffle` | `false` | Write words in random order instead of source order. |
| `seed` | unset | Seed for `shuffle`. Unset → different order every run. Ignored when `shuffle: false`. |

## Input selection

- Only direct children of `input`. Subfolders are not scanned.
- Extension must be exactly `txt`, case-sensitive: `.TXT`, `.text`, `.md` are skipped.
- `output` is skipped if it lies inside `input`, so re-running does not merge the previous
  result into itself.
- Files must be UTF-8; other encodings cause the run to fail. Convert them before merging.
- No matching files → an empty output file, replacing any previous output.

## Cleaning rules

1. `A`–`Z` are lowercased; `a`–`z` are kept.
2. Everything else separates words, including digits, punctuation and accented letters.
3. Words of one letter are dropped — including `a` and `i`: they contain no within-word
   bigrams or trigrams to score. This also excludes them from letter and first-letter stats.
4. Each remaining word is written on its own line. Original line and sentence
   boundaries are lost.

| Source | Output words |
| --- | --- |
| `Hello, World!` | `hello`, `world` |
| `Don't` | `don` (`t` dropped) |
| `e-mail` | `mail` (`e` dropped) |
| `42abc` | `abc` |
| `café` | `caf` |
| `naïve` | `na`, `ve` |

Accented letters split words into fragments, which then enter letter/bigram/trigram stats
as fake words. Use ASCII-only text, or transliterate it before merging.

Punctuation and digits are removed by design: their placement is left to the user, not to
the optimizer.

## Shuffling

`shuffle: false` preserves word order within files, processing files in sorted path order.
`shuffle: true` randomizes word order across all files. Repeated words are kept in both cases.

- Same `seed` + same input files (names and content) → byte-identical output.
- Shuffling does not change corpus stats: `stats` counts letters, bigrams, trigrams and
  first letters inside each word only, so word order is irrelevant to scoring.
- What it does change: the source text can no longer be read back as sentences. The
  vocabulary and word frequencies remain.

## Temp files and disk space

Merge mode is designed for large files and corpora that do not fit in memory. It reads
input incrementally and uses temporary disk buckets to shuffle one portion at a time,
instead of loading the entire corpus into RAM.

- Temp folder: `output` with its extension replaced by `merge.tmp`, next to the output —
  `data/samples/merged.txt` → `data/samples/merged.merge.tmp/`.
- Keep that path reserved: an existing folder there is deleted when merging starts.
- Temporary files are cleaned up on normal exit, errors and Ctrl+C.
- Allow roughly twice the cleaned corpus size in free disk space.

## Interruption

Stopping the run (Ctrl+C) is handled per phase:

| Phase | Result |
| --- | --- |
| Reading input | Stops; output is not touched — a previous `output` stays intact. |
| Writing output | Stops; `output` is left truncated. Re-run to get a complete file. |

## Next steps

Point `stats.input` (and optionally `synthesise.text`) at `merge.output`, then run
`keyvolve -m stats`. See [readme.md](../../readme.md) for the mode list.
