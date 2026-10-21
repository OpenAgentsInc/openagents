#!/usr/bin/env python3
"""A stand-in for scripts/dev/agent-run.py in Pylon's cloud tests."""

import sys

if sys.argv[1:2] == ["labels"]:
    print("run-id=pylon-test,run-host=pylon-test")
sys.exit(0)
