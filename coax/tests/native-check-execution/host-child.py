#!/usr/bin/env python3
"""Controlled subprocess effects for the bounded launcher tests only."""
import json
import os
import sys
import time


mode = sys.argv[1]
if mode == "exact":
    os.write(1, b"x" * int(sys.argv[2]))
elif mode == "flood":
    stream = sys.argv[2]
    for _ in range(1000):
        if stream in ("stdout", "both"):
            os.write(1, b"o" * 512)
        if stream in ("stderr", "both"):
            os.write(2, b"e" * 512)
elif mode == "abort":
    os.abort()
elif mode == "sleep":
    time.sleep(10)
elif mode == "tree":
    child = os.fork()
    if child:
        os.write(1, (json.dumps({"parent": os.getpid(), "child": child}) + "\n").encode())
    time.sleep(10)
else:
    raise SystemExit("unknown fixture mode")
