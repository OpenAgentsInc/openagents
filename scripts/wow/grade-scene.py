#!/usr/bin/env python3
"""Apply a dark chamber grade and soft light bloom to a recorded scene."""
import argparse
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('source', type=Path)
parser.add_argument('output', type=Path)
parser.add_argument('--ffmpeg', default='ffmpeg')
args = parser.parse_args()
if args.source.resolve() == args.output.resolve():
    raise SystemExit('source and output must be different files')
filters = (
    "[0:v]split=2[base][lights];"
    "[base]curves=all='0/0 0.2/0.085 0.5/0.34 0.75/0.70 1/1',"
    "colorbalance=rs=-0.02:bs=0.02:rh=0.055:bh=-0.035,"
    "vignette=PI/7[dark];"
    "[lights]lutrgb=r='if(gt(val,110),(val-110)*1.75,0)':"
    "g='if(gt(val,110),(val-110)*1.75,0)':"
    "b='if(gt(val,110),(val-110)*1.75,0)',gblur=sigma=26[glow];"
    "[dark][glow]blend=all_mode=screen:all_opacity=0.45[out]"
)
subprocess.run([
    args.ffmpeg, '-y', '-i', str(args.source), '-filter_complex', filters,
    '-map', '[out]', '-map', '0:a?', '-c:v', 'libx264', '-preset', 'fast',
    '-crf', '20', '-pix_fmt', 'yuv420p', '-c:a', 'copy',
    '-movflags', '+faststart', str(args.output),
], check=True)
