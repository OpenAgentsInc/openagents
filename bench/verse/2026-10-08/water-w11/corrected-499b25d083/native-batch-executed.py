import json,os,subprocess,sys
from pathlib import Path
base=Path(__file__).resolve().parent; rows=[]
for case in ['low/pond-posts','low/waterline','medium/pond-noon']:
 env=os.environ.copy();env['WATER_W11_CASE']=case
 result=subprocess.run(['python3',str(base/'w11-measure-499b25d083.py'),'native'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
 rows.append({'case':case,'exit':result.returncode});(base/'w11-native-failed-499b25d083-results.json').write_text(json.dumps(rows,indent=2)+'\n')
 print(json.dumps(rows[-1]),flush=True);print(result.stdout[-1200:],flush=True)
sys.exit(any(r['exit'] for r in rows))
