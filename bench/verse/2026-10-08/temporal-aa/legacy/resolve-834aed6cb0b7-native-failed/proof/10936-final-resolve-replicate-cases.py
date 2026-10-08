from pathlib import Path
import subprocess
s=Path(__file__).parent; root=Path('/Users/christopherdavid/.codex/worktrees/bb65/openagents')
for trial in (2,3):
 stem=f'10936-final-resolve-high-replicate-{trial}'
 base=['python3',str(s/f'10936-final-resolve-replicate-{trial}-runner.py'),'--source','834aed6cb0b79c80fbd009c3caa51c78dba06628','--case-index','2','--build-binding',str(s/'10936-final-resolve-build-binding.json')]
 command=['openagents','lease','gpu','--receipt',str(s/(stem+'-gpu.json')),'--','openagents','lease','quiet','--receipt',str(s/(stem+'-quiet.json')),'--']+base+['--run']
 print('Starting fixed replicate '+str(trial),flush=True)
 result=subprocess.run(command,cwd=root)
 if result.returncode: raise SystemExit(result.returncode)
 result=subprocess.run(base+['--finish'],cwd=root)
 if result.returncode: raise SystemExit(result.returncode)
print('Both registered extra trials finished',flush=True)
