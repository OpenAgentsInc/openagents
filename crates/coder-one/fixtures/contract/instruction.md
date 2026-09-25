<!-- a synthetic task for the contract extractor's tests -->

Build a command-line tool at `/app/shout.py` that reads one text file (path passed as the first argument) and prints it in upper case to stdout.

Usage: `python shout.py <text_file>`. The script must exit with a non-zero status code when the argument is missing or when the file does not exist. Output must be the same length as the input. Each invocation must complete within 20 s.

A sample is staged in the environment: `/app/data/sample_text.txt` (input) and `/app/data/sample_shouted.txt` (the corresponding output).

The rule engine is documented in `/app/engine/README.md`. The test suite runs with `make -C /app test` and should pass. The benchmark runs with `make -C /app bench`.

The grader imports `shout` and `VERSION` from `/app/shout.py`.

Each rule has this schema:

```json
{"name": "r1", "from": "a", "to": "b"}
```

Create:

- `/app/rules.json` — a JSON array of rule objects (same schema as above)
- `/app/order.txt` — one rule name per line, in alphabetical order

Write the summary to `/app/out/summary.csv`, one row per file, columns in this order: file, lines, words.

Also save a report at `/app/out/report.tsv` with columns `name`, `score`.

Keep the existing notes at `/app/notes.txt` unchanged, and do not write to `/app/data/forbidden.json`.
