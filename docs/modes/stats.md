# Stats mode - how it works

Stats mode scans a corpus text file once and writes its letter, bigram, trigram and
first-letter frequencies to a JSON file. `evaluate` and `optimize` score layouts against
this file (`corpusStats`), not against the text itself.

## Running

```sh
keyvolve -m stats
```

For configuration overrides and precedence, see the [CLI documentation](../../cliffa/README.md).

## Configuration

```yaml
stats:
  input: data/samples/merged.txt
  output: data/stats/stats.json
  minTrigramFrequency: 0.00001
```

| Key                   | Default      | Meaning                                                                       |
| --------------------- | ------------ | ----------------------------------------------------------------------------- |
| `input`               | - (required) | Corpus text file, usually [`merge.output`](merge.md).                         |
| `output`              | - (required) | Stats JSON. Overwritten if it exists; missing parent folders are created.     |
| `minFrequency`        | `0.000001`   | Drop letters, bigrams and first letters rarer than this share. `0` keeps all. |
| `minTrigramFrequency` | `0.0001`     | Drop trigrams rarer than this share. `0` keeps all.                           |

Missing `input` or `output` → run fails with an error naming the key.

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

Known bug: `average_word_length` currently counts UTF-8 bytes, not characters, so it is
inflated when words contain non-ASCII characters. ASCII input (merge output) is unaffected.
Tracked in [TASKS.md](../../TASKS.md#bugs).

## Frequency filtering

After counting:

1. Entries in `letters`, `bigrams` and `first_letters` below `minFrequency` are dropped.
   Each map is checked separately.
2. Entries in `trigrams` below `minTrigramFrequency` are dropped.
3. Each filtered map is rescaled so its remaining shares sum to `1`.

`word_count` and `average_word_length` are never filtered.

Dropped bigrams and trigrams do not exist for scoring: layouts get no effort or penalty for
them. Raise the thresholds to ignore typos and noise; lower them to keep rare sequences.
A threshold `≤ 0` disables filtering for its maps.

## Output

Pretty-printed JSON. Each map is sorted by share, most frequent first.

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

| Field                               | Used by `evaluate` / `optimize`                                    |
| ----------------------------------- | ------------------------------------------------------------------ |
| `letters`                           | No. Informational only.                                            |
| `bigrams`                           | Yes, scaled to counts by `word_count × (average_word_length - 1)`. |
| `trigrams`                          | Yes, scaled to counts by `word_count × (average_word_length - 2)`. |
| `first_letters`                     | Yes, scaled to counts by `word_count`.                             |
| `average_word_length`, `word_count` | Yes, for the scaling above.                                        |

Point `evaluate.corpusStats` and `optimization.corpusStats` at `output`. A missing file
stops those modes with `Missing corpus stats file: <path>`.

Fitness values already stored in layout CSVs were computed with the old stats. After
rebuilding stats, re-run `evaluate` on those CSVs before comparing them with new results.

## Interruption

Ctrl+C stops reading. `output` is written only after the whole input is counted, so an
interrupted run leaves a previous `output` intact.

Known bug: stats mode currently ignores Ctrl+C and runs to completion. Tracked in
[TASKS.md](../../TASKS.md#bugs).

## Next steps

Set `corpusStats` in `evaluate` and `optimization` to `stats.output`, then run
`keyvolve -m evaluate` ([evaluate.md](../evaluation/evaluate.md)) or `keyvolve -m optimize`.
See [readme.md](../../readme.md) for the mode list.
