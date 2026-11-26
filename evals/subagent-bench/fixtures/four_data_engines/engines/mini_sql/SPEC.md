# Engine 1: `engines.mini_sql`

Implement `MiniDB` in `engines/mini_sql/__init__.py`:
1. `db = MiniDB()`
2. `db.create_table(name: str, rows: list[dict])` stores rows (list of dicts with int/float/str/bool/None values).
3. `db.query(sql: str) -> list[dict]` supports a clean SQL subset (case-insensitive keywords):
   - `SELECT <cols>` where `<cols>` is `*` or comma-separated columns/aggregates with optional `AS alias` (`COUNT(*)`, `SUM(col)`, `AVG(col)`, `MIN(col)`, `MAX(col)`).
   - `FROM <table>` with optional `INNER JOIN <table2> ON <t1.col> = <t2.col>` (support qualified `table.col` or unqualified `col` names).
   - Optional `WHERE <cond>` supporting `=`, `!=`, `<`, `<=`, `>`, `>=`, `AND`, `OR`, parentheses, integer/float/single-quoted string literals.
   - Optional `GROUP BY <col1, ...>`.
   - Optional `ORDER BY <col> [ASC|DESC]` (multiple keys allowed).
   - Optional `LIMIT <n>` and `OFFSET <m>`.
4. Raise `ValueError` on malformed SQL or unknown table.
