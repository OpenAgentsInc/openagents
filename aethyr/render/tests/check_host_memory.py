#!/usr/bin/env python3
# Firm reason: 2 (independent unbounded-integer alignment, allocation-end and page-count oracle for native staging receipts).
"""Normal-C staging ABI against independent unbounded-integer endpoints."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
DRIVER = r'''#include "stage.h"
#include <inttypes.h>
#include <stdio.h>
#include <string.h>
int main(int argc,char**argv){uint64_t cursor,bytes,limit;
 while(scanf("%"SCNu64" %"SCNu64" %"SCNu64,&cursor,&bytes,&limit)==3){
  coax_t_RenderHostStage r=stage_native(cursor,bytes,limit);
  if(argc==2){if(!strcmp(argv[1],"status"))r.status=COAX_RenderFrameStatus_bad_upload;
   if(!strcmp(argv[1],"address"))r.address=0;if(!strcmp(argv[1],"next"))r.next=0;if(!strcmp(argv[1],"pages"))r.pages=0;}
  printf("%u %"PRIu64" %"PRIu64" %"PRIu64"\n",(unsigned)r.status,r.address,r.next,r.pages);
 }return 0;}
'''


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def expected(cursor, size, limit):
    if limit > 67108864:
        return (1, 0, cursor, 0)
    if size % 4:
        return (5, 0, cursor, 0)
    aligned = ((cursor + 3) // 4) * 4
    end = aligned + size
    if aligned > limit or end > limit:
        return (4, 0, cursor, 0)
    return (0, aligned, end, (end + 65535) // 65536)


def main():
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    args = parser.parse_args()
    start = time.monotonic(); work = args.work_dir.resolve(); work.mkdir(parents=True, exist_ok=True)
    compiler = ROOT / "coax/bin/coax"
    def run(argv, **kwargs):
        left = 830 - (time.monotonic() - start)
        assert left > 0, "native staging qualification deadline"
        return subprocess.run(list(map(str, argv)), cwd=ROOT, capture_output=True, check=True, timeout=left, **kwargs)
    compiler_hash = digest(compiler)
    source_digest = run([compiler, "source-digest", ROOT]).stdout.decode().strip()
    assert source_digest == (compiler.parent / "source-digest").read_text().strip()
    fixture = work / "stage.coax"
    fixture.write_text('#load "' + str(ROOT / "aethyr/render/host-memory.coax") + '";\nstage_native :: (c:u64,n:u64,l:u64)->RenderHostStage #export_c {return render_host_stage(c,n,l);}\n')
    driver = work / "driver.c"; driver.write_text(DRIVER)
    deps = json.loads(run([compiler, "deps", fixture, "--library"]).stdout)
    paths = [Path(x) for x in deps["sources"]] + [Path(__file__).resolve(), driver]
    before = {str(path): digest(path) for path in paths}
    maximum = (1 << 64) - 1
    cursors = list(range(21)) + list(range(65531, 65541)) + list(range(67108859, 67108866)) + list(range(maximum - 4, maximum + 1))
    sizes = list(range(9)) + [65532, 65536, 67108860, 67108864, maximum - 3, maximum]
    limits = list(range(13)) + [65535, 65536, 65537, 67108863, 67108864, 67108865, maximum]
    cases = [(cursor, size, limit) for cursor in cursors for size in sizes for limit in limits]
    payload = "".join(f"{c} {n} {l}\n" for c, n, l in cases).encode()
    (work / "inputs.txt").write_bytes(payload)
    oracle = [expected(*case) for case in cases]
    modes = []
    for mode in ("debug", "release", "sanitized"):
        folder = work / mode; folder.mkdir(exist_ok=True)
        header, source = folder / "stage.h", folder / "stage.c"
        emitted = run([compiler, "c", fixture, "--library", "--header", header] + (["--release"] if mode == "release" else []))
        source.write_bytes(emitted.stdout); (folder / "emission.log").write_bytes(emitted.stderr)
        body = re.search(r"struct coax_t_RenderHostStage\s*\{([^}]+)\};", header.read_text())[1]
        fields = [re.search(r"(\w+)\s*$", field.strip())[1] for field in body.split(";") if field.strip()]
        assert fields == ["status", "address", "next", "pages"]
        executable = folder / "stage"
        flags = ["-O2"] if mode == "release" else ["-O0", "-g"]
        if mode == "sanitized": flags += ["-fsanitize=address,undefined", "-fno-omit-frame-pointer"]
        run(["clang", "-std=c11", *flags, "-I", folder, source, driver, "-lm", "-o", executable])
        env = dict(os.environ)
        if mode == "sanitized": env.update(ASAN_OPTIONS="detect_leaks=0:halt_on_error=1", UBSAN_OPTIONS="halt_on_error=1")
        result = run([executable], input=payload, env=env)
        observed = [tuple(map(int, line.split())) for line in result.stdout.splitlines()]
        assert observed == oracle, next(((case, actual, want) for case, actual, want in zip(cases, observed, oracle) if actual != want), "output length differs")
        (folder / "observed.txt").write_bytes(result.stdout)
        controls = []
        for fault in ("status", "address", "next", "pages"):
            altered = run([executable, fault], input=payload, env=env).stdout
            assert altered != result.stdout, "unobserved transported field " + fault
            controls.append(fault)
        modes.append(dict(mode=mode, cases=len(cases), fields=fields, loss_controls=controls,
                          generated_c_sha256=digest(source), header_sha256=digest(header), executable_sha256=digest(executable), transcript_sha256=digest(folder / "observed.txt")))
    assert compiler_hash == digest(compiler)
    assert before == {str(path): digest(path) for path in paths}, "qualification input changed"
    receipt = dict(schema="aethyr.render.host-memory.native.v1", status="pass", compiler_sha256=compiler_hash,
                   compiler_source_digest=source_digest, source_inputs=before, modes=modes,
                   elapsed_seconds=round(time.monotonic() - start, 3),
                   scope="Complete native staging record against Python unbounded-integer alignment/end/page oracles in checked, wrapping and sanitizer modes. Includes four transported-field loss controls per mode; no GPU execution claim.")
    args.receipt.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps({"status": "pass", "cases_per_mode": len(cases), "modes": len(modes)}))


if __name__ == "__main__": main()
