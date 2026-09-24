# Invalid summary-length measurement

These raw records are retained for provenance and spend accounting. The initial
script inspected final response output and missed streamed output items, so its
zero summary lengths are invalid. Its original output-token price was also
wrong. Use `../summary-comparison.json` for corrected costs. No original record
has been silently edited. The valid streamed parser measurements are in
`../summaries-streamed/`.
