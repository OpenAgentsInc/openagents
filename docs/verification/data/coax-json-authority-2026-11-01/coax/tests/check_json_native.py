#!/usr/bin/env python3
"""Normal-C JSON integer/escape boundaries against Python's independent decoder."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
DRIVER = r'''#include "json.h"
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static unsigned nibble(char c){return c<='9'?c-'0':c-'a'+10;}
int main(int argc,char **argv){char op,hex[4097];int cap;
 while(scanf(" %c %d %4096s",&op,&cap,hex)==3){
  if(cap<0||cap>128)return 2;size_t n=!strcmp(hex,"-")?0:strlen(hex)/2;
  char raw[2049]={0};for(size_t i=0;i<n;i++)raw[i]=(char)((nibble(hex[i*2])<<4)|nibble(hex[i*2+1]));
  coax_string text={raw,(int64_t)n};unsigned char output[136];memset(output,0xa5,sizeof output);
  if(op=='I'){int64_t v=json_integer_native(text);if(argc>1&&!strcmp(argv[1],"integer"))v=0;printf("I %"PRId64"\n",v);continue;}
  int64_t count;int bad=0;
  if(op=='E'){count=json_unescape_native(text,(coax_v_u8){output+4,cap});bad=count<0;}
  else if(op=='Q'){coax_t_JsonNativeWrite r=json_requote_native(text,(coax_v_u8){output+4,cap});bad=r.bad;count=bad?-1:r.count;}
  else return 2;
  int guard=1;for(int i=0;i<136;i++)if((i<4||i>=cap+4)&&output[i]!=0xa5)guard=0;
  if(argc>1){if(!strcmp(argv[1],"guard"))guard=0;if(!strcmp(argv[1],"status")){bad=0;count=0;}
   if(!strcmp(argv[1],"byte")&&count>0)output[4]^=1;}
  printf("%c %d %"PRId64" %d ",op,bad,count,guard);
  if(count>0)for(int64_t i=0;i<count;i++)printf("%02x",output[4+i]);else printf("-");puts("");
 }return ferror(stdin)?2:0;}
'''


def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()


def decoded(raw):
    value = json.loads('"' + raw + '"')
    return value.encode("utf-16", "surrogatepass").decode("utf-16", "replace")


def main():
    parser = argparse.ArgumentParser(); parser.add_argument("--work-dir", required=True, type=Path); parser.add_argument("--receipt", required=True, type=Path)
    args = parser.parse_args(); work = args.work_dir.resolve(); work.mkdir(parents=True, exist_ok=True)
    started = time.monotonic(); compiler = ROOT / "coax/bin/coax"; compiler_hash = digest(compiler)
    def run(argv, **kwargs):
        left = 830 - (time.monotonic() - started); assert left > 0
        result = subprocess.run(list(map(str, argv)), cwd=ROOT, capture_output=True, timeout=left, **kwargs)
        if result.returncode: raise RuntimeError(result.stderr.decode(errors="replace")[-4000:])
        return result
    source_digest = run([compiler, "source-digest", ROOT]).stdout.decode().strip()
    assert source_digest == (compiler.parent / "source-digest").read_text().strip()
    fixture = work / "json.coax"
    fixture.write_text(f'''#load "{ROOT}/coax/lib/json/json.coax";
JsonNativeWrite :: struct {{bad:bool;count:int;}}
json_integer_native :: (text:string)->int #export_c {{nodes:[1]JvNode;d:[1]Jv;if !jv_parse(d[..],text,nodes[..]) {{return -71;}}return jv_int(d[0],0,-71);}}
json_unescape_native :: (text:string,out:[]u8)->int #export_c {{return jv_unescape(text,out);}}
json_requote_native :: (text:string,out:[]u8)->JsonNativeWrite #export_c {{o:[1]Jo;jo_start(o[..],out);jo_requote(o[..],text);return JsonNativeWrite.{{bad=o[0].bad,count=o[0].n}};}}
''')
    driver = work / "driver.c"; driver.write_text(DRIVER)
    dependencies = json.loads(run([compiler, "deps", fixture, "--library"]).stdout)
    paths = [Path(name) for name in dependencies["sources"]] + [Path(__file__).resolve(), driver]; before = {str(p): digest(p) for p in paths}
    numbers = [str(n) for n in [0, 1, -1, 9, 10, -10, *(2**53+i for i in range(-12, 13)), *(-2**53+i for i in range(-12, 13)), -(2**63), 2**63-1, 2**63, 10**30]]
    numbers += ["-0", "1e0", "0.0", "false", "null", "01", "+1", "-", "1.", "9"*1000]
    cases = []; expected = []
    for text in numbers:
        try:
            value = json.loads(text); valid = type(value) is int and abs(value) <= 2**53
        except ValueError: valid = False
        cases.append(("I", 0, text)); expected.append("I " + str(value if valid else -71))
    raw = ["", "plain", "héllo", r'\"\\\/\b\f\n\r\t', r"\u0000", r"\u007f", r"\u0080", r"\u07ff", r"\u0800", r"\uffff",
           r"\ud800\udc00x", r"\udbff\udfffz", r"\ud800", r"\udfff", r"\ud800\u0041", r"\udc00\udc00", r"\ud800\ue000",
           "\\", r"\u", r"\u12", r"\u?000", r"\q", r"\x", "\\ud800\\", r"\ud800\uDC0"]
    for text in raw:
        try: value = decoded(text); unescaped = value.encode(); quoted = json.dumps(value, ensure_ascii=False).encode()
        except (ValueError, UnicodeError): unescaped = quoted = None
        for operation, data in (("E", unescaped), ("Q", quoted)):
            for capacity in sorted({0, 1, 2, 3, 4, 7, 8, 16, 128, *(range(max(0,len(data)-1),len(data)+2) if data is not None else [])}):
                bad = data is None or len(data) > capacity; n = -1 if bad else len(data)
                cases.append((operation, capacity, text)); expected.append(f"{operation} {int(bad)} {n} 1 " + (data.hex() if n>0 else "-"))
    payload = "".join(f"{op} {cap} {text.encode().hex() or '-'}\n" for op, cap, text in cases).encode()
    wanted = ("\n".join(expected) + "\n").encode(); (work / "inputs.txt").write_bytes(payload); (work / "oracle.txt").write_bytes(wanted)
    modes = []; debug_source = None
    for mode in ("debug", "release", "sanitized"):
        folder = work / mode; folder.mkdir(exist_ok=True); source = folder / "json.c"; header = folder / "json.h"
        if mode == "sanitized": source.write_bytes(debug_source.read_bytes()); header.write_bytes((debug_source.parent / "json.h").read_bytes())
        else:
            emitted = run([compiler, "c", fixture, "--library", "--header", header] + (["--release"] if mode == "release" else [])); source.write_bytes(emitted.stdout); (folder / "emission.log").write_bytes(emitted.stderr)
            if mode == "debug": debug_source = source
        executable = folder / "json"; flags = ["-O2"] if mode == "release" else ["-O0", "-g"]
        if mode == "sanitized": flags += ["-fsanitize=address,undefined", "-fno-omit-frame-pointer"]
        run(["clang", "-std=c11", *flags, "-I", folder, source, driver, "-lm", "-o", executable]); env = dict(os.environ)
        if mode == "sanitized": env.update(ASAN_OPTIONS="detect_leaks=0:halt_on_error=1", UBSAN_OPTIONS="halt_on_error=1")
        got = run([executable], input=payload, env=env).stdout; (folder / "observed.txt").write_bytes(got)
        assert got == wanted, next(((i, a.decode(), b.decode()) for i, (a,b) in enumerate(zip(got.splitlines(), wanted.splitlines())) if a!=b), "output length differs")
        for loss in ("integer", "status", "byte", "guard"): assert run([executable, loss], input=payload, env=env).stdout != wanted, loss
        modes.append(dict(mode=mode,cases=len(cases),loss_controls=["integer","status","byte","guard"],generated_c_sha256=digest(source),header_sha256=digest(header),executable_sha256=digest(executable),transcript_sha256=digest(folder/"observed.txt")))
    assert compiler_hash == digest(compiler) and before == {str(p):digest(p)for p in paths}, "qualification input changed"
    args.receipt.write_text(json.dumps(dict(schema="coax.json.native.v1",status="pass",compiler_sha256=compiler_hash,compiler_source_digest=source_digest,source_inputs=before,modes=modes,elapsed_seconds=round(time.monotonic()-started,3),scope="Independent Python integer/JSON/Unicode oracle for native exact-integer and raw escape/requote boundaries in checked, release and sanitizer builds, with guarded output spans. Surrogate replacement follows the library's documented policy. Full parser/package transport qualified separately."),indent=2)+"\n")
    print(json.dumps(dict(status="pass",cases_per_mode=len(cases),modes=len(modes))))


if __name__ == "__main__": main()
