#!/bin/bash
# Passes when /app/answer.txt holds what the db sidecar serves, and the
# task's [environment.env] reached the main service.
mkdir -p /logs/verifier
expected=$(python3 -c "import urllib.request; print(urllib.request.urlopen('http://db:8080/ping').read().decode())")
echo "expected: $expected; GREETING=$GREETING; pwd=$(pwd)"
if [ "$GREETING" = hi ] && [ -f /app/answer.txt ] && [ "$(cat /app/answer.txt)" = "$expected" ]; then
  echo 1 > /logs/verifier/reward.txt
else
  echo 0 > /logs/verifier/reward.txt
fi
