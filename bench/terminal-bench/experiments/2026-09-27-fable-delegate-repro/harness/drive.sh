#!/usr/bin/env bash
# Usage: drive.sh <task> <attempt> <deadline>   (issue #9776): preflight, one attempt, check.
set -u
cd ~/fable-delegate-9776
bash pre.sh
avail=$(df --output=avail -BG / | tail -1 | tr -dc 0-9)
[ "$avail" -lt 25 ] && { echo "STOP: disk ${avail}G < 25G"; exit 3; }
echo "START $(date -u +%FT%TZ) $1 $2 $3"
./run.sh "$1" "$2" "$3" > "logs/$1-$2.log" 2>&1
echo "EXIT $? $(date -u +%FT%TZ)"
tail -c 300 "logs/$1-$2.log"
./check.sh "$1" "$2"
