#!/usr/bin/env python3
"""Qualify the shared media ABI against the retained JavaScript arithmetic.

The oracle uses JavaScript Number, the legacy literal-space row decoder and
Float32Array stores. It covers authored whole-pixel rows and a fixed set of
exact decimal inputs; it makes no general correctly-rounded strtod claim.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import random
import struct
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
FIXTURE = '''#load "{root}/aethyr/render/media-commands.coax";
media_native_parse :: (mt:string,mo:[]RenderMediaRequest)->RenderMediaParse #export_c {{return render_media_parse(mt,mo);}}
media_native_rect :: (mq:RenderMediaRequest,mw:int,mh:int,miw:int,mih:int)->RenderMediaRect #export_c {{return render_media_rect(mq,mw,mh,miw,mih);}}
media_native_extent :: (mk:RenderMediaKind,mw:int,mh:int)->RenderMediaExtent #export_c {{return render_media_image_extent(mk,mw,mh);}}
'''
DRIVER = r'''#include "media.h"
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static uint64_t dbits(double d){uint64_t b;memcpy(&b,&d,8);return b;}
static uint32_t fbits(float f){uint32_t b;memcpy(&b,&f,4);return b;}
static double dvalue(uint64_t b){double d;memcpy(&d,&b,8);return d;}
int main(int argc,char **argv){char op;const char *fault=argc>1?argv[1]:"";
 while(scanf(" %c",&op)==1){
  if(op=='P'){
   size_t n;int cap;if(scanf("%zu %d",&n,&cap)!=2||getchar()!='\n'||cap<0||cap>112)return 2;
   char *text=calloc(n+1,1);if(!text||fread(text,1,n,stdin)!=n||getchar()!='\n')return 2;
   coax_t_RenderMediaRequest rows[114];memset(rows,0xa5,sizeof rows);
   coax_t_RenderMediaParse r=media_native_parse((coax_string){text,(int64_t)n},(coax_v_t_RenderMediaRequest){rows+1,cap});
   int guards=1;unsigned char *raw=(unsigned char*)rows;
   for(size_t i=0;i<sizeof rows;i++)if((i<sizeof rows[0]||i>=sizeof rows[0]*(size_t)(r.status?1:r.count+1))&&raw[i]!=0xa5)guards=0;
   if(!strcmp(fault,"parse-count"))r.count=0;
   printf("P %u %"PRId64" %"PRId64" %d %d",(unsigned)r.status,r.count,r.lines,(int)r.truncated,guards);
   for(int64_t i=0;i<r.count;i++){
    coax_t_RenderMediaRequest q=rows[i+1];
    if(!strcmp(fault,"parse-line"))q.line=-1;if(!strcmp(fault,"parse-url"))q.url.count=0;if(!strcmp(fault,"parse-box"))q.box.e[2]=0;
    int borrowed=q.url.data>=text&&q.url.data+q.url.count<=text+n;
    printf(" %u %"PRId64" %d %d %"PRId64" ",(unsigned)q.kind,q.line,(int)q.over,borrowed,q.url.count);
    for(int64_t k=0;k<q.url.count;k++)printf("%02x",(unsigned char)q.url.data[k]);
    if(!q.url.count)printf("-");for(int k=0;k<4;k++)printf(" %016"PRIx64,dbits(q.box.e[k]));
   }puts("");free(text);
  }else if(op=='R'){
   int kind,over;int64_t line,w,h,iw,ih;uint64_t b[4];
   if(scanf("%d %"SCNd64" %d %"SCNd64" %"SCNd64" %"SCNd64" %"SCNd64" %"SCNx64" %"SCNx64" %"SCNx64" %"SCNx64,&kind,&line,&over,&w,&h,&iw,&ih,b,b+1,b+2,b+3)!=11)return 2;
   coax_t_RenderMediaRequest q={0};q.kind=(coax_t_RenderMediaKind)kind;q.line=line;q.over=over;for(int i=0;i<4;i++)q.box.e[i]=dvalue(b[i]);
   coax_t_RenderMediaRect r=media_native_rect(q,w,h,iw,ih);
   if(!strcmp(fault,"rect-line"))r.line=-1;if(!strcmp(fault,"rect-over"))r.over=0;if(!strcmp(fault,"rect-draw"))r.draw=0;
   if(!strncmp(fault,"uniform-",8)){int k=atoi(fault+8);if(k>=0&&k<8)r.uniforms.e[k]=9;}
   printf("R %d %d %"PRId64" %d",(int)r.ok,(int)r.draw,r.line,(int)r.over);for(int i=0;i<8;i++)printf(" %08"PRIx32,fbits(r.uniforms.e[i]));puts("");
  }else if(op=='E'){
   int kind;int64_t w,h;if(scanf("%d %"SCNd64" %"SCNd64,&kind,&w,&h)!=3)return 2;
   coax_t_RenderMediaExtent r=media_native_extent((coax_t_RenderMediaKind)kind,w,h);
   if(!strcmp(fault,"extent-width"))r.width=0;if(!strcmp(fault,"extent-height"))r.height=0;
   printf("E %d %"PRId64" %"PRId64"\n",(int)r.ok,r.width,r.height);
  }else return 2;
 }return ferror(stdin)?2:0;}
'''
ORACLE = r'''const fs=require('node:fs');const c=JSON.parse(fs.readFileSync(0,'utf8'));
const dbits=x=>{const b=Buffer.alloc(8);b.writeDoubleLE(x);return b.readBigUInt64LE().toString(16).padStart(16,'0')};
const fbits=x=>{const b=Buffer.alloc(4);b.writeFloatLE(x);return b.readUInt32LE().toString(16).padStart(8,'0')};
const out=[];
for(const p of c.parse){
 if(Buffer.byteLength(p.text)>65536){out.push('P 1 0 0 0 1');continue}
 const lines=p.text.split('\n'), rows=[];
 for(const [i,line] of lines.slice(0,112).entries()){
  const [named,url,...b]=line.split(' '), box=b.map(Number),kind=named==='over'?'image':named;
  if((kind!=='image'&&kind!=='video')||!url?.startsWith('/')||url.startsWith('//')||box.length!==4||box.some(v=>!Number.isFinite(v)))continue;
  rows.push([kind==='image'?1:2,i,+(named==='over'),1,Buffer.byteLength(url),Buffer.from(url).toString('hex'),...box.map(dbits)].join(' '));
 }
 const status=+(rows.length>p.capacity), count=status?0:rows.length;
 out.push(['P',status,count,Math.min(lines.length,112),+(lines.length>112),1,...(status?[]:rows)].join(' '));
}
for(const r of c.rect){
 const [kind,line,over,w,h,iw,ih,...box]=r, [x0,y0,x1,y1]=box,bw=x1-x0,bh=y1-y0;
 let ok=(kind===1||kind===2)&&line>=0&&line<112&&w>=1&&h>=1&&Number.isFinite(bw)&&Number.isFinite(bh),draw=false,u=Array(8).fill(0);
 if(ok&&!(iw===0&&ih===0)){
  if(iw<1||ih<1)ok=false;
  else if(bw>1&&bh>1){const f=Math.min(bw/iw,bh/ih),dw=iw*f,dh=ih*f,cx=x0+(bw-dw)/2,cy=y0+(bh-dh)/2;
   const v=[cx/w*2-1,1-cy/h*2,(cx+dw)/w*2-1,1-(cy+dh)/h*2];
   ok=v.every(x=>Number.isFinite(x)&&Math.abs(x)<=3.4028234663852886e38);if(ok){draw=true;u=[...v,1,0,0,0]}
  }
 }
 out.push(['R',+ok,+draw,ok?line:-1,ok?+over:0,...u.map(fbits)].join(' '));
}
for(const [kind,w,h] of c.extent){let ok=(kind===1||kind===2)&&w>0&&h>0,x=0,y=0;
 if(ok){const f=kind===2?1:Math.min(1,2048/Math.max(w,h));x=Math.max(1,Math.round(w*f));y=Math.max(1,Math.round(h*f))}
 out.push(['E',+ok,x,y].join(' '));}
process.stdout.write(out.join('\n')+'\n');
'''


def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()


def cases():
    rng = random.Random(110112)
    parse = [dict(text=text, capacity=112) for text in [
        "", "image /x 0 0 320 180", "over /layer -12 4 100 98\nvideo /movie 2 3 300 170",
        "bad\nimage /ok 1 2 3 4\n\nvideo /v 5 6 7 8\n",
        "image /x  0 2 2", "image /x -.5 .25 1e2 0xAf", "video /v 0b11 0o7 1. 2.5e-1",
        "image //unsafe 0 0 2 2\nimage /ok 0 0 2 2 extra\nimage /ok NaN 0 2 2",
        "image / 0 0 2 2", "\n".join(["image /i 0 0 8 8"] * 112),
        "\n".join(["image /i 0 0 8 8"] * 113), "\n" * 112 + "image /late 0 0 8 8",
        "\n" * 112 + "x" * (65536 - 112), "x" * 65537]]
    parse += [dict(text="image /a 0 0 4 4\nvideo /b 1 2 9 8", capacity=n) for n in (0, 1, 2)]
    rect = []
    for i in range(900):
        x, y = rng.randrange(-8192, 8193), rng.randrange(-8192, 8193)
        rect.append([1 + i % 2, i % 112, bool(i % 2), rng.randrange(1, 8193), rng.randrange(1, 8193),
                     rng.randrange(1, 16385), rng.randrange(1, 16385), x, y, x + rng.randrange(-2, 8193), y + rng.randrange(-2, 8193)])
    rect += [[1, 111, True, 400, 200, 400, 400, 10, 20, 210, 120],
             [2, 0, False, 8, 8, 0, 0, 0, 0, 8, 8], [1, 0, True, 8, 8, -1, 2, 0, 0, 8, 8],
             [0, 0, True, 8, 8, 2, 2, 0, 0, 8, 8], [1, 112, True, 8, 8, 2, 2, 0, 0, 8, 8]]
    dimensions = [0, 1, 2, 2047, 2048, 2049, 4096, 8192, 2147483647]
    extent = [[kind, w, h] for kind in (0, 1, 2) for w in dimensions for h in dimensions]
    extent += [[1 + i % 2, rng.randrange(1, 2147483648), rng.randrange(1, 2147483648)] for i in range(1000)]
    return dict(parse=parse, rect=rect, extent=extent)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    args = parser.parse_args(); start = time.monotonic()
    work = args.work_dir.resolve(); work.mkdir(parents=True, exist_ok=True)
    compiler = ROOT / "coax/bin/coax"; compiler_hash = digest(compiler)
    def run(argv, **kw):
        left = 830 - (time.monotonic() - start); assert left > 0
        result = subprocess.run(list(map(str, argv)), cwd=ROOT, capture_output=True, timeout=left, **kw)
        if result.returncode: raise RuntimeError(result.stderr.decode(errors="replace")[-4000:])
        return result
    source_digest = run([compiler, "source-digest", ROOT]).stdout.decode().strip()
    assert source_digest == (compiler.parent / "source-digest").read_text().strip()
    fixture = work / "media.coax"; fixture.write_text(FIXTURE.format(root=ROOT))
    driver = work / "driver.c"; driver.write_text(DRIVER)
    oracle_file = work / "oracle.cjs"; oracle_file.write_text(ORACLE)
    dependencies = json.loads(run([compiler, "deps", fixture, "--library"]).stdout)
    paths = [Path(p) for p in dependencies["sources"]] + [Path(__file__).resolve(), driver, oracle_file]
    before = {str(p): digest(p) for p in paths}
    data = cases(); (work / "cases.json").write_text(json.dumps(data) + "\n")
    payload = bytearray()
    for p in data["parse"]:
        raw = p["text"].encode(); payload += f'P {len(raw)} {p["capacity"]}\n'.encode() + raw + b"\n"
    for r in data["rect"]:
        head = " ".join(str(int(v)) for v in r[:7]); tail = " ".join(struct.pack(">d", float(v)).hex() for v in r[7:])
        payload += f"R {head} {tail}\n".encode()
    for e in data["extent"]: payload += ("E " + " ".join(map(str, e)) + "\n").encode()
    (work / "inputs.txt").write_bytes(payload)
    expected = run(["node", oracle_file], input=json.dumps(data).encode()).stdout
    (work / "oracle.txt").write_bytes(expected)
    modes = []; debug_source = None
    faults = ["parse-count", "parse-line", "parse-url", "parse-box", "rect-line", "rect-over", "rect-draw", *[f"uniform-{i}" for i in range(8)], "extent-width", "extent-height"]
    for mode in ("debug", "release", "sanitized"):
        directory = work / mode; directory.mkdir(exist_ok=True); source = directory / "media.c"; header = directory / "media.h"
        if mode == "sanitized":
            source.write_bytes(debug_source.read_bytes()); header.write_bytes((debug_source.parent / "media.h").read_bytes())
        else:
            emitted = run([compiler, "c", fixture, "--library", "--header", header] + (["--release"] if mode == "release" else []))
            source.write_bytes(emitted.stdout); (directory / "emission.log").write_bytes(emitted.stderr)
            if mode == "debug": debug_source = source
        executable = directory / "media"; flags = ["-O2"] if mode == "release" else ["-O0", "-g"]
        if mode == "sanitized": flags += ["-fsanitize=address,undefined", "-fno-omit-frame-pointer"]
        run(["clang", "-std=c11", "-ffp-contract=off", "-fno-fast-math", *flags, "-I", directory, source, driver, "-lm", "-o", executable])
        env = dict(os.environ)
        if mode == "sanitized": env.update(ASAN_OPTIONS="detect_leaks=0:halt_on_error=1", UBSAN_OPTIONS="halt_on_error=1")
        result = run([executable], input=payload, env=env).stdout; (directory / "observed.txt").write_bytes(result)
        if result != expected:
            actual_rows, wanted_rows = result.splitlines(), expected.splitlines()
            raise AssertionError(next(((i, a.decode(), b.decode()) for i, (a, b) in enumerate(zip(actual_rows, wanted_rows)) if a != b), "output length differs"))
        for fault in faults:
            assert run([executable, fault], input=payload, env=env).stdout != expected, "unobserved field " + fault
        modes.append(dict(mode=mode, cases=sum(map(len, data.values())), loss_controls=faults,
                          generated_c_sha256=digest(source), header_sha256=digest(header), executable_sha256=digest(executable), transcript_sha256=digest(directory / "observed.txt")))
    assert compiler_hash == digest(compiler) and before == {str(p): digest(p) for p in paths}, "qualification input changed"
    args.receipt.write_text(json.dumps(dict(schema="aethyr.render.media.native.v1", status="pass", compiler_sha256=compiler_hash,
                                          compiler_source_digest=source_digest, source_inputs=before, oracle_sha256=digest(oracle_file),
                                          modes=modes, elapsed_seconds=round(time.monotonic() - start, 3),
                                          scope="Native borrowed media records, atomic capacity, line window, exact f32 aspect-fit uniforms and bounded static image extents against independent JavaScript Number/Float32Array arithmetic. Authored integer and selected exact decimal rows only; no image decoding or universal numeric-parser rounding claim."), indent=2) + "\n")
    print(json.dumps(dict(status="pass", cases_per_mode=sum(map(len, data.values())), modes=len(modes))))


if __name__ == "__main__": main()
