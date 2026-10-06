#!/usr/bin/env python3
import os,sys
args=sys.argv[1:]
name=args[args.index('--crate-name')+1] if '--crate-name' in args else ''
if name in ['verse_engine','verse_world','verse_pbr','verse_imported']:
 args=[('incremental=/run/verse-audit-build-agent1/incremental' if x=='incremental=/home/christopherdavid/work/openagents-target-agent1/debug/incremental' else x) for x in args]
if '--test' in args and name in ['verse_pbr','verse_imported']:args.extend(['-C','linker=/run/verse-audit-build-agent1/cc.py'])
os.execv(args[0],args)
