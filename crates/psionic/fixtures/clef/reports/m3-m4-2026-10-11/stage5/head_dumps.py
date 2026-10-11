import hashlib, json, os, pathlib, signal, subprocess, sys, time
def interrupt(signum, frame):
 raise KeyboardInterrupt(signum)
signal.signal(signal.SIGTERM,interrupt)
repo=pathlib.Path("repo")
sys.path.insert(0,str(repo/"crates/psionic/fixtures/clef/tools"))
from mac_quiet_bench import OwnedProcessGuard, available_memory, file_sha
root=pathlib.Path(__file__).parent
binary=root/"psionic-serve-tests"
base={k:v for k,v in os.environ.items() if not k.startswith("PSIONIC_CLEF_")}
base.update(RAYON_NUM_THREADS="2",OMP_NUM_THREADS="2",PSIONIC_CLEF_GGUF=str(root.parent/"artifacts/Clef-Q4_K_M.gguf"),PSIONIC_CLEF_TEST_DEVICE="metal",PSIONIC_CLEF_CUDA_CHECK="1",PSIONIC_CLEF_HF_HEAD=str(root.parent/"artifacts/clef-bf16/joint_head.safetensors"),PSIONIC_CLEF_METAL_FFN_WEIGHTS="q4k-on-demand")
checks=[
("native27b-head-014", "clef::tests::dump_head_parity_inputs", {"PSIONIC_CLEF_DUMP_RECORD":"14", "PSIONIC_CLEF_DUMP_DIR":str(root/"native27b-head-014/inputs")}, False),
("native27b-head-034", "clef::tests::dump_head_parity_inputs", {"PSIONIC_CLEF_DUMP_RECORD":"34", "PSIONIC_CLEF_DUMP_DIR":str(root/"native27b-head-034/inputs")}, False)]
rows=[]
digest=file_sha(binary)
for name, test, changes, ignored in checks:
 out=root/name
 out.mkdir(exist_ok=False)
 command=[str(binary),test,"--test-threads=1","--nocapture"]
 if ignored: command.append("--ignored")
 if name!="serve-gpu": command.append("--exact")
 before=available_memory()
 if before["available_memory_bytes"]<64*1024**3: raise RuntimeError("less than 64GiB available before bounded model run")
 started=time.monotonic(); process=guard=None
 row={"name":name,"command":command,"binary_sha256":digest,"environment":changes,"conditions":"shared machine; correctness only","preflight":before}
 print("Starting "+name,flush=True)
 try:
  with (out/"output.log").open("wb") as log:
   process=subprocess.Popen(command,env={**base,**changes},stdout=log,stderr=log,start_new_session=True)
   guard=OwnedProcessGuard(process,52*1024**3,out/"memory.jsonl",minimum_available_memory_bytes=12*1024**3)
   while process.poll() is None:
    guard.guard()
    if time.monotonic()-started>900: raise TimeoutError("own test exceeded15minutes")
    time.sleep(.2)
   guard.guard(); row["exit"]=process.returncode
 except BaseException as error:
  row.update(error=str(error),exit=1)
 finally:
  if guard:
   try: guard.close()
   except BaseException as error: row.update(cleanup_error=str(error),exit=1)
   row["memory"]=guard.receipt()
  row["seconds"]=time.monotonic()-started
  (out/"receipt.json").write_text(json.dumps(row,indent=2)+"\n")
 rows.append(row)
 (root/"native27b-head-checks.json").write_text(json.dumps(rows,indent=2)+"\n")
 print(json.dumps(row),flush=True)
 print((out/"output.log").read_text()[-6500:],flush=True)
 if row.get("exit")!=0: raise SystemExit(1)
