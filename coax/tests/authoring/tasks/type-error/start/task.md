# Repair type errors

`mean.coax` doesn't compile: `coax test` reports its type errors. Make it
compile without changing what each procedure means (see the comments): a mean
in f64, a count of frames over the budget, and that count's share.

Edit only `mean.coax`. Check with `python3 coax/tests/authoring.py check DIR`.
