import json, os, pathlib, subprocess, hashlib
root=pathlib.Path('/run/verse-audit-build-agent1/v26-public-proof')
root.mkdir(exist_ok=True)
repo=pathlib.Path('/home/christopherdavid/work/openagents-verse-audit')
target=pathlib.Path('/home/christopherdavid/work/openagents-target-agent1')
toolchain=pathlib.Path('/home/christopherdavid/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu')
cargo=pathlib.Path('/home/christopherdavid/.cargo')
bash=pathlib.Path('/run/current-system/sw/bin/bash').resolve()
system=pathlib.Path('/run/current-system/sw').resolve()
python=pathlib.Path('/home/christopherdavid/.nix-profile/bin/python3').resolve().parent
base=['bwrap','--unshare-all','--die-with-parent','--ro-bind','/nix','/nix','--proc','/proc','--dev','/dev','--tmpfs','/tmp','--ro-bind','/run/current-system/sw/bin/env','/usr/bin/env','--ro-bind',str(system),'/run/current-system/sw','--ro-bind',str(toolchain),str(toolchain),'--ro-bind',str(repo),str(repo),'--bind',str(target),str(target),'--bind','/run/verse-audit-build-agent1','/run/verse-audit-build-agent1','--dir',str(cargo),'--ro-bind',str(cargo/'registry'),str(cargo/'registry'),'--ro-bind',str(cargo/'git'),str(cargo/'git'),'--setenv','HOME','/tmp/empty-home','--setenv','CARGO_HOME',str(cargo),'--setenv','CARGO_TARGET_DIR',str(target),'--setenv','RUSTC',str(toolchain/'bin/rustc'),'--setenv','RUSTC_WRAPPER','/run/verse-audit-build-agent1/rustc.py','--setenv','CARGO_BUILD_JOBS','1','--setenv','TMPDIR','/tmp','--setenv','PATH',str(toolchain/'bin')+':'+str(python)+':/run/current-system/sw/bin','--chdir',str(repo)]
script = 'set -euo pipefail\n'
script += 'test ! -e /home/christopherdavid/work/ruins\n'
script += 'test ! -e /home/christopherdavid/work/wow\n'
script += 'test ! -e /home/christopherdavid/.openagents\n'
script += 'test ! -e /home/christopherdavid/work/coder\n'
script += 'test ! -e /home/christopherdavid/work/bender\n'
script += 'mkdir -p /tmp/empty-home\n'
script += 'cargo build --offline --locked -p verse-content --features compiler,verse-world/service-net,verse-world/studio --bin verse-content\n'
script += str(target/'debug/verse-content')+' ritual '+str(root/'ritual')+'\n'
script += str(target/'debug/verse-content')+' author init '+str(root/'ritual')+' '+str(root/'workspace')+' isolated-public-world\n'
script += str(target/'debug/verse-content')+' author release '+str(root/'workspace')+' '+str(root/'release')+'\n'
command=base+['--',str(bash),'-c',script]
(root/'namespace.json').write_text(json.dumps({'schema':'verse.public-release.namespace.v1','network':'unshared','private_game_directories':'absent; only public checkout, toolchain, compiler caches, target, and scratch output are mounted','argv':command},indent=2)+'\n')
with (root/'namespace.log').open('w') as log:
 result=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT)
print(json.dumps({'exit_code':result.returncode,'log':str(root/'namespace.log')}))
raise SystemExit(result.returncode)
