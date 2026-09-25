#!/bin/sh
# No Python heredoc: nothing to split.
cd /app || exit 1
n=0
test -f out.csv && n=$((n+1))
grep -q total out.csv && n=$((n+1))
echo "SCORE $n 2"
