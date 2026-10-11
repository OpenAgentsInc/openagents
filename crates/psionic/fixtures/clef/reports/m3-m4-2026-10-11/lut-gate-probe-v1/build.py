#!/usr/bin/env python3
"""Compile the scratch projection probe without executing it."""
from pathlib import Path
import subprocess
p=Path(__file__).resolve().parent
commands=[['xcrun','-sdk','macosx','metal','-std=metal4.0','-c',str(p/'baseline.metal'),'-o',str(p/'baseline.air')],
 ['xcrun','-sdk','macosx','metallib',str(p/'baseline.air'),'-o',str(p/'baseline.metallib')],
 ['xcrun','clang','-O2','-fobjc-arc','-fno-fast-math','-framework','Foundation','-framework','Metal','-framework','MetalPerformanceShaders',str(p/'probe.m'),'-o',str(p/'probe')]]
for command in commands:subprocess.run(command,check=True)
