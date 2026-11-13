# Firm reason: 2 (an independent oracle: the same kernels in C, C++ and Rust, built and timed beside Coax's for the Episode 296 comparison)
import subprocess,time,sys,os
C=os.environ.get('COAX','coax/bin/coax')  # run from the folder bench-gen.py wrote into
def t(cmd,n=3):
    best=1e9
    for _ in range(n):
        a=time.time(); r=subprocess.run(cmd,capture_output=True,text=True,timeout=120); d=time.time()-a
        if r.returncode!=0: return 'FAIL '+(r.stderr or r.stdout)[-200:]
        best=min(best,d)
    return round(best,3)
which=sys.argv[1]
if which=='run':
    for name,cmd in [('coax checked -O0 (default)',[C,'build','k.coax','-o','k1']),('coax checked -O2',[C,'build','k.coax','--opt','2','-o','k2']),('coax --release (wrapping, -O2)',[C,'build','k.coax','--release','-o','k3']),
        ('C clang -O2',['clang','-O2','k.c','-o','k4']),('C clang -O2 no-fma',['clang','-O2','-ffp-contract=off','k.c','-o','k8']),('C clang -O0',['clang','-O0','k.c','-o','k5']),('Rust -O',['rustc','-O','k.rs','-o','k6']),('Rust debug',['rustc','k.rs','-o','k7'])]:
        b=t(cmd,1); i=name; exe='./'+cmd[-1]
        print(f"{name:34} build {b}  run {t([exe],7)}",flush=True)
else:
    for name,cmd in [('coax check (front end only)',[C,'check','big.coax']),('coax lib (checked, -O0)',[C,'lib','big.coax','-o','b1.o']),('coax lib --opt 2 (checked)',[C,'lib','big.coax','--opt','2','-o','b2.o']),('coax lib --release',[C,'lib','big.coax','--release','-o','b2r.o'])]:
        print(f"{name:30} {t(cmd,3)}",flush=True)
