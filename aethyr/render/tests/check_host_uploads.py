#!/usr/bin/env python3
"""Check the native upload retry receipt against integer prefix-set unions."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
DRIVER = r'''#include "uploads.h"
#include <inttypes.h>
#include <stdio.h>
#include <string.h>
int main(int argc,char**argv){uint64_t prior,incoming,capacity;int submitted;
 while(scanf("%"SCNu64" %"SCNu64" %"SCNu64" %d",&prior,&incoming,&capacity,&submitted)==4){
  coax_t_RenderHostUploadState before={.pending=prior};
  coax_t_RenderHostUploadReceipt staged=uploads_stage_native(before,incoming,capacity);
  coax_t_RenderHostUploadState after=uploads_ack_native(staged.next,submitted);
  if(argc==2){if(!strcmp(argv[1],"status"))staged.status=COAX_RenderFrameStatus_bad_upload;
   if(!strcmp(argv[1],"pending"))staged.next.pending=0;if(!strcmp(argv[1],"grew"))staged.grew=0;if(!strcmp(argv[1],"ack"))after.pending=0;}
  printf("%u %"PRIu64" %d %"PRIu64"\n",(unsigned)staged.status,staged.next.pending,(int)staged.grew,after.pending);
 }return ferror(stdin)?2:0;}
'''


def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()


def oracle(prior, incoming, capacity, submitted):
    status = 5 if prior % 4 or incoming % 4 else 4 if max(prior, incoming) > capacity else 0
    prefix = prior if status else max(prior, incoming)
    return (status, prefix, int(not status and incoming > prior), 0 if submitted else prefix)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    args = parser.parse_args(); work = args.work_dir.resolve(); work.mkdir(parents=True, exist_ok=True)
    started = time.monotonic(); compiler = ROOT / "coax/bin/coax"; compiler_hash = digest(compiler)
    def run(argv, **kwargs):
        left = 830 - (time.monotonic() - started); assert left > 0
        result = subprocess.run(list(map(str, argv)), cwd=ROOT, capture_output=True, timeout=left, **kwargs)
        if result.returncode: raise RuntimeError(result.stderr.decode(errors="replace")[-4000:])
        return result
    source_digest = run([compiler, "source-digest", ROOT]).stdout.decode().strip()
    assert source_digest == (compiler.parent / "source-digest").read_text().strip()
    fixture = work / "uploads.coax"
    fixture.write_text(f'#load "{ROOT}/aethyr/render/host-uploads.coax";\nuploads_stage_native :: (s:RenderHostUploadState,n:u64,c:u64)->RenderHostUploadReceipt #export_c {{return render_host_upload_stage(s,n,c);}}\nuploads_ack_native :: (s:RenderHostUploadState,a:bool)->RenderHostUploadState #export_c {{return render_host_upload_ack(s,a);}}\n')
    driver = work / "driver.c"; driver.write_text(DRIVER)
    dependencies = json.loads(run([compiler, "deps", fixture, "--library"]).stdout)
    paths = [Path(name) for name in dependencies["sources"]] + [Path(__file__).resolve(), driver]
    before = {str(path): digest(path) for path in paths}
    maximum = (1 << 64) - 1
    endpoints = list(range(13)) + [65532, 67108864, 1 << 32, 1 << 53, maximum - 7, maximum - 3, maximum]
    limits = list(range(14)) + [65532, 67108864, 1 << 32, 1 << 53, maximum - 3, maximum]
    cases = [(a, b, c, s) for a in endpoints for b in endpoints for c in limits for s in (False, True)]
    payload = "".join(f"{a} {b} {c} {int(s)}\n" for a, b, c, s in cases).encode()
    (work / "inputs.txt").write_bytes(payload); expected = [oracle(*case) for case in cases]
    modes = []; debug_source = None
    for mode in ("debug", "release", "sanitized"):
        folder = work / mode; folder.mkdir(exist_ok=True); source = folder / "uploads.c"; header = folder / "uploads.h"
        if mode == "sanitized":
            source.write_bytes(debug_source.read_bytes()); header.write_bytes((debug_source.parent / "uploads.h").read_bytes())
        else:
            emitted = run([compiler, "c", fixture, "--library", "--header", header] + (["--release"] if mode == "release" else []))
            source.write_bytes(emitted.stdout); (folder / "emission.log").write_bytes(emitted.stderr)
            if mode == "debug": debug_source = source
        flags = ["-O2"] if mode == "release" else ["-O0", "-g"]
        if mode == "sanitized": flags += ["-fsanitize=address,undefined", "-fno-omit-frame-pointer"]
        executable = folder / "uploads"; run(["clang", "-std=c11", *flags, "-I", folder, source, driver, "-lm", "-o", executable])
        env = dict(os.environ)
        if mode == "sanitized": env.update(ASAN_OPTIONS="detect_leaks=0:halt_on_error=1", UBSAN_OPTIONS="halt_on_error=1")
        observed = run([executable], input=payload, env=env).stdout; (folder / "observed.txt").write_bytes(observed)
        actual = [tuple(map(int, row.split())) for row in observed.splitlines()]
        assert actual == expected, next(((case, got, want) for case, got, want in zip(cases, actual, expected) if got != want), "output length differs")
        for field in ("status", "pending", "grew", "ack"):
            assert run([executable, field], input=payload, env=env).stdout != observed, "unobserved field " + field
        modes.append(dict(mode=mode, cases=len(cases), loss_controls=["status", "pending", "grew", "ack"],
                          generated_c_sha256=digest(source), header_sha256=digest(header), executable_sha256=digest(executable), transcript_sha256=digest(folder / "observed.txt")))
    assert compiler_hash == digest(compiler) and before == {str(path): digest(path) for path in paths}, "qualification input changed"
    args.receipt.write_text(json.dumps(dict(schema="aethyr.render.host-uploads.native.v1", status="pass", compiler_sha256=compiler_hash,
                                          compiler_source_digest=source_digest, source_inputs=before, modes=modes, elapsed_seconds=round(time.monotonic() - started, 3),
                                          scope="Complete transported upload union/growth/ack receipt against independent unbounded-integer prefix sets. Includes large native u64 endpoints and four field-loss controls per mode. GPU submission truth belongs to the actual effects gate."), indent=2) + "\n")
    print(json.dumps(dict(status="pass", cases_per_mode=len(cases), modes=len(modes))))


if __name__ == "__main__": main()
