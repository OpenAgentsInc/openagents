import json, os, pathlib, subprocess, time
root=pathlib.Path(__file__).parent
env={k:v for k,v in os.environ.items() if not k.startswith("PSIONIC_CLEF_")}
commands=[("metal", ["cargo","test","--release","--manifest-path","crates/psionic/Cargo.toml","-p","psionic-backend-metal","--lib","clef","--","--test-threads=1"]), ("serve", ["cargo","test","--release","--manifest-path","crates/psionic/Cargo.toml","-p","psionic-serve","--lib","clef","--","--test-threads=1"])]
rows=[]
for name,command in commands:
 print("Starting "+name,flush=True)
 started=time.monotonic()
 with (root/(name+".log")).open("w") as out:
  result=subprocess.run(command,env=env,stdout=out,stderr=subprocess.STDOUT)
 row={"name":name,"command":command,"exit":result.returncode,"seconds":time.monotonic()-started}
 rows.append(row)
 (root/"checks.json").write_text(json.dumps(rows,indent=2)+"\n")
 print(json.dumps(row),flush=True)
 print("\n".join((root/(name+".log")).read_text().splitlines()[-55:]),flush=True)
 if result.returncode: raise SystemExit(result.returncode)
