import sys
# ---- runtime kernels
open('k.coax','w').write('''#import "Basic";
N :: 1_000_000;
POLY :: cast(u32) 0xEDB88320;
make_table :: () -> [256] u32 {
  table: [256] u32;
  for n: 0..<256 {
    c := cast(u32) n;
    for k: 0..<8 { c = ifx c & 1 == 1 then POLY ^ (c >> 1) else c >> 1; }
    table[n] = c;
  }
  return table;
}
TABLE :: #run make_table();
crc32 :: (data: [] u8) -> u32 {
  crc: u32 = 0xFFFF_FFFF;
  for byte: data { crc = TABLE[(crc ^ cast(u32) byte) & 0xFF] ^ (crc >> 8); }
  return crc ^ 0xFFFF_FFFF;
}
sieve :: (flags: [] u8) -> int {
  for i: 0..<flags.count { flags[i] = 1; }
  count := 0;
  for i: 2..<flags.count {
    if flags[i] == 1 {
      count += 1;
      j := i * 2;
      while j < flags.count { flags[j] = 0; j += i; }
    }
  }
  return count;
}
escape :: (cr: f64, ci: f64) -> int {
  zr := 0.0; zi := 0.0;
  for n: 0..<500 {
    if zr * zr + zi * zi > 4 { return n; }
    t := zr * zr - zi * zi + cr;
    zi = 2 * zr * zi + ci;
    zr = t;
  }
  return 500;
}
main :: () {
  flags: [N] u8;
  primes := 0; crc: u32 = 0;
  for r: 0..<60 { primes += sieve(flags[..]); crc = crc ^ crc32(flags[..]); }
  inside := 0;
  for row: 0..<1000 { for col: 0..<2000 {
    cr := -2.2 + 3.0 * cast(f64) col / 2000;
    ci := -1.2 + 2.4 * cast(f64) row / 1000;
    if escape(cr, ci) == 500 { inside += 1; }
  } }
  print("% % %\\n", primes, crc, inside);
}
''')
open('k.c','w').write('''#include <stdio.h>
#include <stdint.h>
#include <stddef.h>
#define N 1000000
static uint32_t TABLE[256];
static void make_table(void){for(int n=0;n<256;n++){uint32_t c=n;for(int k=0;k<8;k++)c=(c&1)?0xEDB88320u^(c>>1):c>>1;TABLE[n]=c;}}
static uint32_t crc32(const uint8_t*d,size_t n){uint32_t crc=0xFFFFFFFFu;for(size_t i=0;i<n;i++)crc=TABLE[(crc^d[i])&0xFF]^(crc>>8);return crc^0xFFFFFFFFu;}
static long sieve(uint8_t*f,long n){for(long i=0;i<n;i++)f[i]=1;long count=0;for(long i=2;i<n;i++){if(f[i]==1){count++;for(long j=i*2;j<n;j+=i)f[j]=0;}}return count;}
static long escape(double cr,double ci){double zr=0,zi=0;for(long n=0;n<500;n++){if(zr*zr+zi*zi>4)return n;double t=zr*zr-zi*zi+cr;zi=2*zr*zi+ci;zr=t;}return 500;}
int main(void){static uint8_t flags[N];make_table();long primes=0;uint32_t crc=0;
for(int r=0;r<60;r++){primes+=sieve(flags,N);crc^=crc32(flags,N);}
long inside=0;for(long row=0;row<1000;row++)for(long col=0;col<2000;col++){double cr=-2.2+3.0*(double)col/2000;double ci=-1.2+2.4*(double)row/1000;if(escape(cr,ci)==500)inside++;}
printf("%ld %u %ld\\n",primes,crc,inside);return 0;}
''')
open('k.rs','w').write('''const N: usize = 1_000_000;
fn make_table() -> [u32; 256] { let mut t=[0u32;256]; for n in 0..256 { let mut c=n as u32; for _ in 0..8 { c = if c&1==1 {0xEDB88320 ^ (c>>1)} else {c>>1}; } t[n]=c; } t }
fn crc32(t:&[u32;256], d:&[u8]) -> u32 { let mut crc=0xFFFF_FFFFu32; for &b in d { crc = t[((crc ^ b as u32)&0xFF) as usize] ^ (crc>>8); } crc ^ 0xFFFF_FFFF }
fn sieve(f:&mut [u8]) -> i64 { for i in 0..f.len() { f[i]=1; } let mut count=0; for i in 2..f.len() { if f[i]==1 { count+=1; let mut j=i*2; while j<f.len() { f[j]=0; j+=i; } } } count }
fn escape(cr:f64, ci:f64) -> i64 { let (mut zr, mut zi)=(0.0f64,0.0f64); for n in 0..500 { if zr*zr+zi*zi>4.0 { return n; } let t=zr*zr-zi*zi+cr; zi=2.0*zr*zi+ci; zr=t; } 500 }
fn main(){ let t=make_table(); let mut flags=vec![0u8;N]; let mut primes=0i64; let mut crc=0u32;
 for _ in 0..60 { primes+=sieve(&mut flags); crc^=crc32(&t,&flags); }
 let mut inside=0i64; for row in 0..1000 { for col in 0..2000 { let cr=-2.2+3.0*(col as f64)/2000.0; let ci=-1.2+2.4*(row as f64)/1000.0; if escape(cr,ci)==500 { inside+=1; } } }
 println!("{} {} {}", primes, crc, inside); }
''')
# ---- scale: M functions
M=int(sys.argv[1])
co=['#import "Basic";']; c=['#include <stdio.h>','#include <stdint.h>']; cpp=['#include <cstdio>','#include <cstdint>','#include <array>']; rs=[]
for k in range(M):
    a,b,m=k%7+2,k%11+3,k%13+5
    co.append(f'''fn_{k} :: (x: int, y: int) -> int {{
  buf: [16] int;
  for i: 0..<16 {{ buf[i] = (x * {a} + i * {b} + y) % 1009; }}
  acc := 0;
  for v, i: buf {{
    if v % {m} == 0 {{ acc += v * {a}; }} else {{ acc += v + i; }}
  }}
  return acc % 100003;
}}''')
    c.append(f'''static long fn_{k}(long x,long y){{
  long buf[16];
  for(int i=0;i<16;i++){{ buf[i]=(x*{a}+i*{b}+y)%1009; }}
  long acc=0;
  for(int i=0;i<16;i++){{ long v=buf[i];
    if(v%{m}==0){{ acc+=v*{a}; }} else {{ acc+=v+i; }}
  }}
  return acc%100003;
}}''')
    cpp.append(f'''static long fn_{k}(long x,long y){{
  std::array<long,16> buf;
  for(int i=0;i<16;i++){{ buf[i]=(x*{a}+i*{b}+y)%1009; }}
  long acc=0; int i=0;
  for(long v: buf){{
    if(v%{m}==0){{ acc+=v*{a}; }} else {{ acc+=v+i; }}
    i++;
  }}
  return acc%100003;
}}''')
    rs.append(f'''fn fn_{k}(x: i64, y: i64) -> i64 {{
  let mut buf = [0i64; 16];
  for i in 0..16 {{ buf[i] = (x * {a} + (i as i64) * {b} + y) % 1009; }}
  let mut acc = 0i64;
  for (i, v) in buf.iter().enumerate() {{
    if v % {m} == 0 {{ acc += v * {a}; }} else {{ acc += v + i as i64; }}
  }}
  acc % 100003
}}''')
G=30
def drivers(head,call,ret,tail):
    out=[]
    for g in range(0,M,G):
        out.append(head.format(g=g//G)+'\n'+'\n'.join(call.format(k=k) for k in range(g,min(g+G,M)))+'\n'+ret+'\n'+tail)
    return out
ng=(M+G-1)//G
co+=drivers('drv_{g} :: (t0: int) -> int #export_c #effects(all) {{\n  t := t0;','  t = (t + fn_{k}(t, {k})) % 1000003;','  return t;','}')
c+=drivers('long drv_{g}(long t){{','  t=(t+fn_{k}(t,{k}))%1000003;','  return t;','}')
cpp+=drivers('long drv_{g}(long t){{','  t=(t+fn_{k}(t,{k}))%1000003;','  return t;','}')
rs+=drivers('pub fn drv_{g}(mut t: i64) -> i64 {{','  t=(t+fn_{k}(t,{k}))%1000003;','  t','}')
for n,x in [('big.coax',co),('big.c',c),('big.cpp',cpp),('big.rs',rs)]: open(n,'w').write('\n'.join(x)+'\n')
