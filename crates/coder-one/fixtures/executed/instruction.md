The `tally` package in `/app` sums the numbers in a file and prints their
mean, but the mean is wrong. Fix it.

Run `python3 -m tally data/numbers.txt` to see the sum and the mean; it
must succeed. `python3 -m tally` with no argument exits non-zero.

Also write `/app/report.sh`, a script that prints one line saying what you
changed; `sh report.sh` must succeed.
