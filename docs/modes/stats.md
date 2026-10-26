# Stats mode - how it works

Stats mode scans a corpus text file once and writes its letter, bigram, trigram and
first-letter frequencies to `stats.json` in the output folder. `evaluate` and `optimize`
score layouts against this file (`corpusStats`), not against the text itself.

## Running

```sh
keyvolve -m stats
```

For configuration overrides and precedence, see the [CLI documentation](../../cliffa/README.md).

## Configuration

```yaml
stats:
  input: data/samples/merged.txt
  output: data/stats
  minTrigramFrequency: 0.00001
```

| Key                   | Default      | Meaning                                                                                       |
| --------------------- | ------------ | --------------------------------------------------------------------------------------------- |
| `input`               | - (required) | Corpus text file, usually [`merge.output`](merge.md).                                         |
| `output`              | - (required) | Output folder. Created if missing; existing files with the same names are overwritten.        |
| `minTrigramFrequency` | `0.0001`     | Drop trigrams rarer than this share. `0` keeps all.                                           |

Missing `input` or `output` → run fails with an error naming the key. `output` pointing at
an existing file → run fails before reading input.

## Input

- Words are split on ASCII whitespace (space, tab, line breaks). Line breaks have no other
  meaning.
- No cleaning: case, digits, punctuation and accented letters are counted as-is. `Hello,`
  is one word with letters `H`, `e`, `l`, `l`, `o`, `,`. Run [merge](merge.md) first to
  get lowercase `a`-`z` words.
- Each line is read whole into memory. Merge output (one word per line) is safe; a huge
  single-line file is not.
- Empty input → empty frequency maps, `average_word_length: 0`, `word_count: 0`.
  `evaluate` and `optimize` then score every layout against nothing.

## What is counted

Counting happens inside each word only. Pairs and triples across word boundaries are never
counted, so word order does not matter.

| Metric                | Counted per word                              | Share of            |
| --------------------- | --------------------------------------------- | ------------------- |
| `letters`             | every character                               | all characters      |
| `bigrams`             | every adjacent pair (`the` → `th`, `he`)      | all bigrams         |
| `trigrams`            | every adjacent triple (`then` → `the`, `hen`) | all trigrams        |
| `first_letters`       | first character                               | all words           |
| `average_word_length` | length in characters                          | mean over all words |

Single-letter words add to `letters`, `first_letters` and `average_word_length` only;
two-letter words add no trigrams.

`average_word_length` exists because the cache stores normalized frequencies, but the
optimizer needs approximate raw counts when it rebuilds a corpus model from the JSON. The
code converts the cache back to counts using the mean word length:

- `word_count × average_word_length` ≈ total characters
- `word_count × (average_word_length - 1)` ≈ total bigrams
- `word_count × (average_word_length - 2)` ≈ total trigrams

Without it, the evaluator could not turn probabilities back into counts for the layout
score.

Example input `the tea a`:

| Metric                | Result                             |
| --------------------- | ---------------------------------- |
| `letters`             | `t` 2/7, `e` 2/7, `a` 2/7, `h` 1/7 |
| `bigrams`             | `th`, `he`, `te`, `ea` 1/4 each    |
| `trigrams`            | `the`, `tea` 1/2 each              |
| `first_letters`       | `t` 2/3, `a` 1/3                   |
| `average_word_length` | 7/3 ≈ 2.33                         |
| `word_count`          | 3                                  |

## Frequency filtering

After counting:

1. `letters`, `bigrams` and `first_letters` are left untouched; all entries are counted.
2. Entries in `trigrams` below `minTrigramFrequency` are dropped.
3. Each filtered map is rescaled so its remaining shares sum to `1`.

`word_count` and `average_word_length` are never filtered.

Dropped trigrams do not exist for scoring: layouts get no effort or penalty for them.
Lower the threshold to keep rare sequences; raise it to ignore noise.
A threshold `≤ 0` disables trigram filtering.

## Output

All files go into the `output` folder:

| File                     | Content                                                            |
| ------------------------ | ------------------------------------------------------------------ |
| `stats.json`             | Stats used by `evaluate` / `optimize`.                             |
| `letters.csv`            | Letter frequencies, one row per letter.                            |
| `bigrams.csv`            | Ordered bigrams, so `th` and `ht` stay distinct.                   |
| `bigrams.aggregated.csv` | Aggregated bigrams, where `th` and `ht` are merged into one total. |
| `first-letters.csv`      | First-letter frequencies, one row per letter.                      |

`stats.json` is pretty-printed JSON. Each map is sorted by share, most frequent first.
Shares are fractions (`0`-`1`).

```json
{
  "stats": {
    "letters": { "e": 0.1197, "t": 0.0879 },
    "bigrams": { "th": 0.0352 },
    "trigrams": { "the": 0.0218 },
    "first_letters": { "t": 0.1601 },
    "average_word_length": 4.98
  },
  "word_count": 3465110798
}
```

Each CSV has `key,share,count` columns, sorted by share, most frequent first. `share` is the
JSON share as a percentage with 6 decimals (`11.970000%`), so Excel reads it as a percent.
`count` is the estimated raw count for that metric (`word_count × average_word_length`,
`word_count × (average_word_length - 1)`, etc.). These files are meant for quick inspection
in Excel or a text editor without opening the JSON.

| Field                               | Used by `evaluate` / `optimize`                                    |
| ----------------------------------- | ------------------------------------------------------------------ |
| `letters`                           | No. Informational only.                                            |
| `bigrams`                           | Yes, scaled to counts by `word_count × (average_word_length - 1)`. |
| `trigrams`                          | Yes, scaled to counts by `word_count × (average_word_length - 2)`. |
| `first_letters`                     | Yes, scaled to counts by `word_count`.                             |
| `average_word_length`, `word_count` | Yes, for the scaling above.                                        |

Point `evaluate.corpusStats` and `optimization.corpusStats` at `<output>/stats.json`. A
missing file stops those modes with `Missing corpus stats file: <path>`.

Fitness values already stored in layout CSVs were computed with the old stats. After
rebuilding stats, re-run `evaluate` on those CSVs before comparing them with new results.

## Interruption

Ctrl+C stops reading. Files are written only after the whole input is counted, so an
interrupted run leaves previous files in `output` intact.

## Next steps

Set `corpusStats` in `evaluate` and `optimization` to `<stats.output>/stats.json`, then run
`keyvolve -m evaluate` ([evaluate.md](../evaluation/evaluate.md)) or `keyvolve -m optimize`.
See [readme.md](../../readme.md) for the mode list.
