#!/usr/bin/env python3
import pathlib,subprocess,sys,os,shutil
args=sys.argv[1:]
if '-o' not in args:os.execvp('cc',['cc',*args])
i=args.index('-o')+1
original=pathlib.Path(args[i]);root=pathlib.Path('/run/verse-audit-build-agent1/linked');root.mkdir(exist_ok=True)
output=root/original.name
assert not output.exists(),str(output)
args[i]=str(output)
result=subprocess.run([shutil.which('cc'),*args])
if result.returncode==0:
 assert output.is_file()
 original.symlink_to(output)
sys.exit(result.returncode)
