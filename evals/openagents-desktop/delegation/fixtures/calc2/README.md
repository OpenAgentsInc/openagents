# calc

`evaluate(text)` in `calc.py` returns the value of an arithmetic expression, with the same result
Python gives for the same expression.

- Numbers: integers (`42`), decimals (`3.5`, `.5`, `2.`) and exponents (`1e3`, `2.5e-1`).
- Operators: `+ - * / // % **`, unary `+` and `-`, and parentheses. Spaces are allowed anywhere
  between tokens.
- Precedence and grouping are Python's: `**` groups right to left and binds tighter than a unary
  minus on its left (`-2**2` is `-4`), while its right side may start with a unary operator
  (`2**-1` is `0.5`).
- Types are Python's: `7 // 2` is the int `3`, `6 / 3` is the float `2.0`, `2 ** 3` is the int `8`.
- A division by zero raises `ZeroDivisionError`. Anything that is not a valid expression (empty
  text, `2 +`, `(1`, `1 2`, `2 ** * 3`, a name, any other character) raises `ValueError`.

`calc.py` must do the parsing itself. It must not use `eval`, `exec`, `compile` or the `ast` module.
