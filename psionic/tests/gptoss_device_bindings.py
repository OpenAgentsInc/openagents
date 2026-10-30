#!/usr/bin/env python3
"""Generate test-only native invocation glue from the emitted kernel ABI."""
import argparse,json,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
def main():
    p=argparse.ArgumentParser();p.add_argument('header',type=Path);a=p.parse_args();text=a.header.read_text();plan=(ROOT/'psionic/lib/gptoss/device-plan.coax').read_text()
    names=json.loads('['+re.search(r'GD_NAMES :: \[\d+\]string\.\[(.*?)\];',plan,re.S)[1]+']');names=[n for n in names if n]
    native={'s64':'int64_t','u64':'uint64_t','u32':'uint32_t','u16':'uint16_t','u8':'uint8_t','f32':'float','f64':'double'}
    print('/* Test-only dispatch: exactly the exported Coax kernel signatures. */')
    print('static void pg_cpu_launch(PgSession *s,int kind,int64_t item,const coax_t_GDArg *a){switch(kind){')
    for i,name in enumerate(names):
        signature=re.search(r'void '+name+r'\((.*?)\);',text)[1].split(', ');assert signature[0]=='int64_t item';arguments=['item']
        for j,parameter in enumerate(signature[1:]):
            typ=parameter.split()[0]
            if typ=='int64_t':arguments.append(f'a[{j}].value')
            else:
                assert typ.startswith('coax_v_');c=native[typ.removeprefix('coax_v_')]
                arguments.append(f'({typ}){{({c}*)pg_address(s,a[{j}].ref),a[{j}].ref.count}}')
        print(f'case {i}: {name}('+','.join(arguments)+');break;')
    print('default: abort();}}')
    # Which parameters are f64 views: float-float pairs on Metal, binary64 here.
    print('static inline int pg_cpu_f64(int kind,int parameter){switch(kind){')
    for i,name in enumerate(names):
        signature=re.search(r'void '+name+r'\((.*?)\);',text)[1].split(', ')[1:]
        wide=[j for j,parameter in enumerate(signature) if parameter.split()[0]=='coax_v_f64']
        if wide:print(f'case {i}: return '+' || '.join(f'parameter=={j}' for j in wide)+';')
    print('default: return 0;}}')
if __name__=='__main__':main()
