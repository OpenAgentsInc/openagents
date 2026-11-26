# Plugin: `plugins.csv_norm`

Implement `normalize_csv(text: str, required_cols: list[str] | None = None) -> tuple[list[str], list[dict[str, str]]]` in `plugins/csv_norm/__init__.py`:
1. Parse RFC-4180 CSV supporting quoted fields with embedded commas, escaped double quotes (`""`), and embedded newlines inside quoted fields. Handle both `\r\n` and `\n` line endings and optional UTF-8 BOM (`\ufeff`) at start.
2. Normalize header names by stripping surrounding whitespace and lowercasing; raise `ValueError` if headers are empty or duplicated after normalization.
3. Skip completely blank rows (rows where all cells are empty/whitespace).
4. Strip leading/trailing whitespace from each unquoted or quoted cell value.
5. If a data row has fewer cells than headers, pad missing values with `""`; if more cells than headers, raise `ValueError`.
6. If `required_cols` is provided, normalize them the same way and raise `ValueError` if any required column is missing from headers.
7. Return `(headers, rows)` where each row is a `dict[str, str]` mapping normalized header -> value.
