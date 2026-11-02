#!/usr/bin/env python3
"""Independent closed-cube geometry oracle for normal checked-C output."""
from pathlib import Path
import argparse,hashlib,json,math,os,struct,subprocess,time

ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def f32(x):return struct.unpack('<f',struct.pack('<f',x))[0]
def word(x):return struct.unpack('<I',struct.pack('<f',x))[0]
def sub(a,b):return tuple(x-y for x,y in zip(a,b))
def cross(a,b):return(a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0])
def dot(a,b):return sum(x*y for x,y in zip(a,b))

def expected():
    # Classic independently authored outward cube: eight dyadic corners,
    # twelve triangles, then real affine transformation and cofactor normals.
    corners=[(-.5,-.5,-.5),(.5,-.5,-.5),(.5,.5,-.5),(-.5,.5,-.5),(-.5,-.5,.5),(.5,-.5,.5),(.5,.5,.5),(-.5,.5,.5)]
    triangles=[1,2,6,1,6,5,0,4,7,0,7,3,3,7,6,3,6,2,0,1,5,0,5,4,4,5,6,4,6,7,0,3,2,0,2,1]
    matrices=[[(1,0,0,0),(0,1,0,0),(0,0,1,0)],[(0,0,2,3),(0,3,0,-2),(-4,0,0,5)],[(2,0,0,3),(0,4,0,-2),(0,0,8,5)]]
    normals=[(1,0,0),(-1,0,0),(0,1,0),(0,-1,0),(0,0,1),(0,0,-1)]
    packets=[]
    for matrix in matrices:
        cols=list(zip(*[r[:3]for r in matrix]));cof=[cross(cols[1],cols[2]),cross(cols[2],cols[0]),cross(cols[0],cols[1])]
        assert dot(cols[0],cof[0])>0
        for i,corner in enumerate(triangles):
            p=corners[corner];w=tuple(dot(row[:3],p)+row[3]for row in matrix)
            n=tuple(sum(cof[j][axis]*normals[i//6][j]for j in range(3))for axis in range(3));n=(n[0],n[1]+f32(.00001),n[2]);m=math.sqrt(dot(n,n));n=tuple(f32(x/m)for x in n)
            packets.append(tuple(map(f32,(*w,1,*w,*n))))
    return packets

def validate(data):
    want=expected()
    if len(data)!=len(want)*40:return False,'packet count'
    got=[struct.unpack_from('<10f',data,i*40)for i in range(len(want))]
    for i,(a,b)in enumerate(zip(got,want)):
        for lane,(x,y)in enumerate(zip(a,b)):
            if not math.isfinite(x)or(word(x)!=word(y)and not(x==0 and y==0)):
                return False,f'packet {i} lane {lane}: {x} != {y}'
    for model in range(3):
        for triangle in range(12):
            a,b,c=got[model*36+triangle*3:model*36+triangle*3+3]
            if dot(cross(sub(b[4:7],a[4:7]),sub(c[4:7],a[4:7])),a[7:10])<=0:return False,'inward triangle'
    return True,None

def main():
    p=argparse.ArgumentParser();p.add_argument('--coax',type=Path,required=True);p.add_argument('--work',type=Path,required=True);a=p.parse_args();a.work.mkdir(exist_ok=True)
    start=time.monotonic();fixture=ROOT/'aethyr/tests/render-box36-native.coax';compiler=a.coax.resolve();deps=json.loads(subprocess.check_output([compiler,'deps',fixture],cwd=ROOT,timeout=120))['sources']
    source_paths=[Path(x)for x in deps]+[Path(__file__),HERE/'box36-native.c',compiler];before={str(x):sha(x)for x in source_paths}
    c,h=a.work/'Box36.c',a.work/'Box36.h';command=[str(compiler),'c',str(fixture),'--library','--header',str(h)]
    with c.open('wb')as out,(a.work/'emission.log').open('wb')as err:r=subprocess.run(command,cwd=ROOT,stdout=out,stderr=err,timeout=180)
    assert r.returncode==0,(a.work/'emission.log').read_text()[-2000:]
    modes=[]
    for mode,flags in [('debug',['-O0']),('o2',['-O2']),('san',['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'])]:
        exe=a.work/('native-'+mode);compile_command=['clang','-std=c11',*flags,'-ffp-contract=off','-fno-fast-math','-I',str(a.work),str(c),str(HERE/'box36-native.c'),'-lm','-o',str(exe)]
        r=subprocess.run(compile_command,capture_output=True,timeout=120);(a.work/(mode+'-compile.log')).write_bytes(r.stdout+r.stderr);assert r.returncode==0,r.stderr.decode()[-2000:]
        image=a.work/(mode+'.packets');environment=dict(os.environ,ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1')
        r=subprocess.run([exe,image],capture_output=True,env=environment,timeout=30);(a.work/(mode+'.log')).write_bytes(r.stdout+r.stderr);assert r.returncode==0,r.stderr.decode()[-2000:]
        data=image.read_bytes();ok,error=validate(data);assert ok,error
        words=list(struct.unpack('<'+str(len(data)//4)+'I',data));controls=[]
        faults=[]
        changed=words.copy();changed[10:20],changed[20:30]=changed[20:30],changed[10:20];faults.append(('reversed-triangle',struct.pack('<'+str(len(changed))+'I',*changed)))
        changed=words.copy();changed[4]=word(-.5);faults.append(('lost-face-plane',struct.pack('<'+str(len(changed))+'I',*changed)))
        changed=words.copy();changed[7]=word(-1);faults.append(('reversed-normal',struct.pack('<'+str(len(changed))+'I',*changed)))
        faults.append(('missing-last-vertex',data[:-40]))
        changed=bytearray(data)
        for model in range(3):
            for face in range(4):
                at=(model*36+face*6)*40
                old=[data[at+i*40:at+(i+1)*40]for i in range(6)]
                for i,j in enumerate([0,5,2,3,4,1]):changed[at+i*40:at+(i+1)*40]=old[j]
        faults.append(('historical-inward-order',bytes(changed)))
        for name,bad in faults:
            detected,why=validate(bad);assert not detected,name;controls.append({'fault':name,'detected':True,'reason':why})
        modes.append({'mode':mode,'packets':108,'channels':1080,'triangles':36,'exact_nonzero_f32_words':True,'signed_zero_equivalent':True,'controls':controls,'command':compile_command,'executable_sha256':sha(exe),'output_sha256':sha(image)})
    after={str(x):sha(x)for x in source_paths};assert after==before,'source changed'
    record={'schema':'aethyr.render.box36-native-primitive.v1','status':'pass','scope':'Actual production sceneVS under identity viewport and three independent positive-determinant affine object transforms. Classic outward cube oracle covers all36positions/faceplanes/authored normals plus12triangles per model. Exact nonzero binary32 words; signedzero equivalent. This is not full-frame numerical acceptance or a culling change.',
        'source_sha256':before,'source_changes':[],'compiler_sha256':sha(compiler),'emission_command':command,'generated_c_sha256':sha(c),'header_sha256':sha(h),'modes':modes,'elapsed_seconds':time.monotonic()-start}
    (a.work/'qualification.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({'status':'pass','modes':3,'packets_per_mode':108,'loss_controls_per_mode':5,'elapsed_seconds':record['elapsed_seconds']}))

if __name__=='__main__':main()
