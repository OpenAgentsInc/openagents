# Engine 3: `engines.diff_engine`

Implement line-level LCS/Myers diff and unified patch engine in `engines/diff_engine/__init__.py` (do NOT import `difflib`):
1. `diff_ops(old_lines: list[str], new_lines: list[str]) -> list[tuple[str, str]]`:
   - Computes a minimal edit script of `("=", line)`, `("-", line)`, `("+", line)` tuples using dynamic programming / Myers shortest edit script.
2. `make_unified_patch(old_text: str, new_text: str, from_file: str = "a", to_file: str = "b", context: int = 3) -> str`:
   - Emits standard unified diff format with `--- <from_file>`, `+++ <to_file>`, and `@@ -l,s +l,s @@` hunks with `context` surrounding unchanged lines (merging overlapping/adjacent hunks). Empty string if texts are identical.
3. `apply_unified_patch(old_text: str, patch_text: str) -> str`:
   - Parses unified hunks, validates context & removed lines against `old_text` (raising `ValueError` on mismatch / corrupt hunk), and returns the exact patched text (round-tripping `make_unified_patch` cleanly, including empty files and trailing newlines).
