#!/usr/bin/env python3
"""Compile only the scratch capability executable; never runs it."""
from pathlib import Path
import subprocess
root = Path(__file__).resolve().parent
subprocess.run(['xcrun', 'clang', '-O2', '-fobjc-arc', '-fno-fast-math',
                '-framework', 'Foundation', '-framework', 'Metal',
                '-framework', 'MetalPerformanceShaders', str(root / 'probe.m'),
                '-o', str(root / 'probe')], check=True)
