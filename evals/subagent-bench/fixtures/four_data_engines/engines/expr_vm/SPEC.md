# Engine 2: `engines.expr_vm`

Implement a tokenizer, recursive-descent parser, bytecode compiler, and stack VM in `engines/expr_vm/__init__.py`:
1. `compile_expr(source: str) -> list[tuple]` compiles expressions into bytecode instructions (e.g. `("PUSH", val)`, `("LOAD", name)`, `("STORE", name)`, `("ADD",)`, etc.) and returns the non-empty instruction list.
2. `run_vm(bytecode: list[tuple], env: dict | None = None)` executes bytecode and returns the final value.
3. `eval_expr(source: str, env: dict | None = None)` compiles & runs `source`.
4. Language features required:
   - Numbers (int & float), booleans (`true`, `false`), identifiers, parentheses.
   - Operators with standard precedence: unary `-`, `!`/`not`; `*`, `/`, `//`, `%`, `**` (right-assoc); `+`, `-`; comparisons `==`, `!=`, `<`, `<=`, `>`, `>=`; logical `and`/`&&`, `or`/`||` (short-circuiting!).
   - Conditional expression: `if <cond> then <expr1> else <expr2>`.
   - Local bindings: `let <name> = <expr1> in <expr2>` (lexically scoped, can shadow outer bindings without mutating caller `env`).
   - Built-in calls: `abs(x)`, `min(a, b, ...)`, `max(a, b, ...)`, `clamp(x, lo, hi)`.
5. Raise `ValueError` on syntax errors, unknown variables, or division by zero.
