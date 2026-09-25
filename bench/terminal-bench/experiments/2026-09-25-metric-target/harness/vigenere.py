"""Harness for a run-time target, written from the stated definition only:
one invocation of the cracker on the staged development ciphertext, timed.

Usage: python3 vigenere.py. Prints `METRIC <seconds>`.
"""

import subprocess
import sys
import time

start = time.perf_counter()
done = subprocess.run(
    [sys.executable, "/app/cracker.py", "/app/data/sample_ciphertext.txt"],
    stdout=subprocess.DEVNULL,
    check=False,
)
elapsed = time.perf_counter() - start
print("exit", done.returncode)
print("METRIC", elapsed)
sys.exit(done.returncode)
