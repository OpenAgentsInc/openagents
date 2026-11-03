#!/usr/bin/env python3
"""New M3 aggregate IEEE/ABI/order corpus; never runs the previous quantity corpus."""
from pathlib import Path
import hashlib,json,os,platform,random,re,struct,subprocess,tempfile,time
ROOT=Path(__file__).resolve().parents[2]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax')).resolve()
SOURCE=ROOT/'coax/tests/spatial/native.coax'
RAW=ROOT/'coax/tests/spatial/native-raw.coax'
NATIVE=ROOT/'coax/tests/spatial-native.c'
SEED=0x53334d333031
STEMS=('point','vector','second','pp','pvadd','vpadd','pvsub','vvadd','vvsub','vmul','smul','vdiv','vneg','secondadd','alias')

def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def patterns():
    edge32=[0,0x80000000,1,0x80000001,0x007fffff,0x807fffff,0x00800000,0x80800000,
            0x3f800000,0xbf800000,0x3f000000,0xbf000000,0x40000000,0xc0000000,
            0x3f800001,0x3f7fffff,0x7f7fffff,0xff7fffff,0x7f800000,0xff800000,
            0x7fc00001,0xffc12345,0x33800000,0xb3800000]
    edge64=[0,0x8000000000000000,1,0x8000000000000001,0x000fffffffffffff,0x800fffffffffffff,
            0x0010000000000000,0x8010000000000000,0x3ff0000000000000,0xbff0000000000000,
            0x3fe0000000000000,0xbfe0000000000000,0x4000000000000000,0xc000000000000000,
            0x3ff0000000000001,0x3fefffffffffffff,0x7fefffffffffffff,0xffefffffffffffff,
            0x7ff0000000000000,0xfff0000000000000,0x7ff8000000000001,0xfff8123456789abc,
            0x3ca0000000000000,0xbca0000000000000]
    rng=random.Random(SEED)
    def make(edges,width,exponent,mantissa,quiet):
        count=len(edges)
        rows=[(edges[i],edges[(i+7)%count],edges[(i+13)%count],edges[j],edges[(j+11)%count],edges[(j+17)%count],edges[(5*i+7*j)%count]) for i in range(count) for j in range(count)]
        for _ in range(512):
            row=[]
            for _ in range(7):
                word=rng.getrandbits(width)
                if word&exponent==exponent and word&mantissa:word|=quiet
                row.append(word)
            rows.append(tuple(row))
        return rows
    return make(edge32,32,0x7f800000,0x007fffff,0x00400000),make(edge64,64,0x7ff0000000000000,0x000fffffffffffff,0x0008000000000000)

def function_text(text,name):
    found=re.search(r'(?m)^(?:static )?[A-Za-z_][A-Za-z_0-9]*\s+'+re.escape(name)+r'\([^;\n]*\)\s*\{',text)
    assert found,('missing generated function',name)
    at=text.index('{',found.start());cursor=at+1;depth=1
    while depth:
        if text[cursor]=='{':depth+=1
        if text[cursor]=='}':depth-=1
        cursor+=1
    return '\n'.join(line for line in text[found.start():cursor].splitlines() if not line.lstrip().startswith('#line'))

def aliases(header):
    def prototype(name):
        found=re.search(r'(?m)^([A-Za-z_][A-Za-z_0-9]*)\s+'+re.escape(name)+r'\(([^;\n]*)\);',header)
        assert found,('missing exported prototype',name)
        return found.group(1),found.group(2)
    declarations=['#include "spatial-native.h"']
    mapping={}
    for width in (32,64):
        for short,name in [('P','point'),('V','vector'),('S','second'),('A','arrays')]:mapping[short+str(width)]=prototype('SPN_'+name+str(width))[0]
        args=prototype('SPN_view'+str(width))[1].split(',')
        assert len(args)==2
        for short,param in zip(('PV','VV'),args):
            found=re.fullmatch(r'\s*([A-Za-z_][A-Za-z_0-9]*)\s+[A-Za-z_][A-Za-z_0-9]*\s*',param)
            assert found,('unexpected view prototype',param)
            mapping[short+str(width)]=found.group(1)
    mapping['Record']=prototype('SPN_record')[0]
    for alias,name in mapping.items():declarations.append('typedef '+name+' '+alias+';')
    return '\n'.join(declarations)+'\n',mapping

def macho_functions(path,names):
    """Read exact Mach-O object symbol extents; do not normalize instructions/relocations."""
    data=Path(path).read_bytes()
    if len(data)<32 or struct.unpack_from('<I',data)[0]!=0xfeedfacf:
        return {'available':False,'reason':'This focused artifact reader supports little-endian Mach-O64 objects only.'}
    _,cpu,subcpu,kind,ncmds,command_bytes,flags,reserved=struct.unpack_from('<IIIIIIII',data)
    assert kind==1 and 32+command_bytes<=len(data)
    cursor=32;sections=[];symbols=None
    for _ in range(ncmds):
        cmd,size=struct.unpack_from('<II',data,cursor);assert size>=8 and cursor+size<=len(data)
        if cmd==0x19:
            segment=struct.unpack_from('<II16sQQQQiiII',data,cursor);nsects=segment[9]
            assert 72+80*nsects<=size
            for i in range(nsects):
                fields=struct.unpack_from('<16s16sQQIIIIIIII',data,cursor+72+80*i)
                sections.append({'name':fields[0].split(b'\0')[0].decode(),'segment':fields[1].split(b'\0')[0].decode(),'address':fields[2],'size':fields[3],'offset':fields[4],'relocation_offset':fields[6],'relocation_count':fields[7]})
        if cmd==2:symbols=struct.unpack_from('<IIIIII',data,cursor)[2:]
        cursor+=size
    assert symbols is not None
    text_index=next(i+1 for i,s in enumerate(sections) if s['name']=='__text' and s['segment']=='__TEXT');text=sections[text_index-1]
    symoff,nsyms,stroff,strsize=symbols;strings=data[stroff:stroff+strsize];table=[]
    for i in range(nsyms):
        strx,typ,sect,desc,value=struct.unpack_from('<IBBHQ',data,symoff+16*i)
        assert strx<len(strings)
        end=strings.find(b'\0',strx);assert end>=strx
        table.append({'name':strings[strx:end].decode(),'type':typ,'section':sect,'value':value})
    defined=[s for s in table if s['type']&0x0e==0x0e and s['section']==text_index]
    boundaries=sorted(set([s['value'] for s in defined]+[text['address']+text['size']]))
    relocations=[]
    for i in range(text['relocation_count']):
        address,info=struct.unpack_from('<iI',data,text['relocation_offset']+8*i);external=bool(info&(1<<27));index=info&0xffffff
        relocation={'address':address,'info':info,'external':external,'symbol':table[index]['name'] if external and index<len(table) else None}
        relocations.append(relocation)
    functions={}
    for name in names:
        entry=next((s for s in defined if s['name']=='_'+name),None)
        if entry is None:functions[name]={'available':False,'reason':'export not present in object symbol table'};continue
        first=entry['value'];end=next(v for v in boundaries if v>first);begin=first-text['address'];length=end-first
        code=data[text['offset']+begin:text['offset']+begin+length];assert len(code)==length
        rs=[r for r in relocations if begin<=r['address']<begin+length]
        functions[name]={'available':True,'bytes':length,'hex':code.hex(),'sha256':hashlib.sha256(code).hexdigest(),'relocations':rs,'relocation_count':len(rs)}
    return {'available':True,'format':'Mach-O64','cpu_type':cpu,'cpu_subtype':subcpu,'extent_scope':'Exact symbol-to-next-text-symbol bytes, including any trailing alignment; relocation-bearing byte equality alone is not an instruction-equivalence claim.','functions':functions}

def main():
    assert os.sys.byteorder=='little','owned case/observation transport is little endian'
    requested=os.environ.get('COAX_SPATIAL_RECORD_DIR')
    work=Path(requested) if requested else Path(tempfile.mkdtemp(prefix='coax-spatial-native-'))
    if work.exists():assert work.is_dir() and not any(work.iterdir()),'record directory must be new or empty'
    else:work.mkdir(parents=True)
    rows32,rows64=patterns();assert len(rows32)==len(rows64)==1088
    data=struct.pack('<II',len(rows32),len(rows64))+b''.join(struct.pack('<7I',*row) for row in rows32)+b''.join(struct.pack('<7Q',*row) for row in rows64)
    (work/'cases.bin').write_bytes(data)
    source_paths=[SOURCE,RAW,NATIVE,Path(__file__).resolve()];source_hashes={str(p.relative_to(ROOT)):sha(p) for p in source_paths}
    names=['SPN_'+stem+str(width) for width in (32,64) for stem in STEMS]
    record={'schema':'coax-spatial-native-v1','seed':SEED,'compiler':str(COMPILER),'compiler_sha256':sha(COMPILER),'platform':platform.platform(),'machine':platform.machine(),'cases_f32':len(rows32),'cases_f64':len(rows64),'numeric_f32_per_mode':39*len(rows32)+54,'numeric_f64_per_mode':39*len(rows64)+60,'input_sha256':sha(work/'cases.bin'),'source_sha256':source_hashes,'commands':[],'modes':[],'body_comparison':{},'object_comparison':{},'passed':False,
            'oracle':'Independent volatile raw C coordinate add/subtract/multiply/divide/negate, aggregate-copy/ABI guards and explicit effect trace literals; raw Coax is used only for code-generation comparison, never as the numeric oracle.',
            'profile':'Round-to-nearest host IEEE binary32/binary64, no fast math/contraction; require AArch64 FZ and x86 SSE FTZ/DAZ off. Exact non-NaN bits including signed zero/subnormals/infinities; NaNs by classification with canonical transcript. NaN payloads/exception flags and cross-target determinism are outside scope.',
            'scope':'NEW Point3/Vector3 concrete same-frame/unit/rep affine arithmetic; raw three-field physical aggregate, function-pointer, nested/fixed-array/view ABI; one-time left-to-right whole operands and left snapshot before right-side view mutation. Foreign scalar interruption is a host effect-order probe, not recognized typed Coax trap evidence. No foreign aggregate widening, transforms, angular interpretation, epochs, device ABI, models/SDKs, full R12 or old corpora.'}
    def save(): (work/'receipt.json').write_text(json.dumps(record,indent=2)+'\n')
    def command(name,args):
        begin=time.monotonic();result=subprocess.run([str(v) for v in args],cwd=ROOT,capture_output=True,timeout=180)
        (work/(name+'.stdout')).write_bytes(result.stdout);(work/(name+'.stderr')).write_bytes(result.stderr)
        record['commands'].append({'name':name,'arguments':[str(v) for v in args],'cwd':str(ROOT),'exit':result.returncode,'duration_seconds':time.monotonic()-begin,'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest(),'stderr_bytes':len(result.stderr)});save()
        assert result.returncode==0,(name,result.stdout.decode(errors='replace'),result.stderr.decode(errors='replace'))
        assert not result.stderr,(name,'unexpected stderr',result.stderr.decode(errors='replace'))
        return result
    began=time.monotonic()
    try:
        record['compiler_version']=command('compiler-version',[COMPILER,'version']).stdout.decode().strip();cc=os.environ.get('CC','cc');record['c_compiler_version']=command('cc-version',[cc,'--version']).stdout.decode()
        command('coax-checks',[COMPILER,'test',SOURCE])
        profiles={}
        for profile,extra in [('debug',[]),('release',['--release'])]:
            directory=work/profile;directory.mkdir()
            typed=command('typed-'+profile,[COMPILER,'c',SOURCE,'--library',*extra,'--header',directory/'spatial-native.h']).stdout.decode();(directory/'typed.c').write_text(typed)
            raw=command('raw-'+profile,[COMPILER,'c',RAW,'--library',*extra]).stdout.decode();(directory/'raw.c').write_text(raw)
            alias_text,mapping=aliases((directory/'spatial-native.h').read_text());(directory/'spatial-types.h').write_text(alias_text)
            profiles[profile]={'path':directory,'typed':typed,'raw':raw};record.setdefault('generated_profiles',{})[profile]={'C_sha256':sha(directory/'typed.c'),'header_sha256':sha(directory/'spatial-native.h'),'raw_C_sha256':sha(directory/'raw.c'),'private_typedef_mapping':mapping}
            record['body_comparison'][profile]={}
            for name in names:
                a=function_text(typed,name);b=function_text(raw,name)
                record['body_comparison'][profile][name]={'exact_without_line_directives':a==b,'typed_sha256':hashlib.sha256(a.encode()).hexdigest(),'raw_sha256':hashlib.sha256(b.encode()).hexdigest(),'typed_lexical_call_tokens':sorted(set(re.findall(r'\b([A-Za-z_][A-Za-z_0-9]*)\s*\(',a.split('{',1)[1]))),'raw_lexical_call_tokens':sorted(set(re.findall(r'\b([A-Za-z_][A-Za-z_0-9]*)\s*\(',b.split('{',1)[1])))}
                (directory/(name+'-typed.txt')).write_text(a+'\n');(directory/(name+'-raw.txt')).write_text(b+'\n')
        base=['-std=c11','-Wall','-Wextra','-Werror','-Wno-unused-function','-Wno-unused-variable','-Wno-unused-parameter','-Wno-misleading-indentation','-Wno-unknown-pragmas','-ffp-contract=off','-fno-fast-math']
        outputs=[]
        for mode,profile,flags in [('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            directory=profiles[profile]['path'];obj=work/(mode+'-typed.o')
            command('compile-'+mode+'-object',[cc,*base,*flags,'-I'+str(directory),'-c',directory/'typed.c','-o',obj])
            command('compile-'+mode+'-native',[cc,*base,*flags,'-I'+str(directory),obj,NATIVE,'-lm','-o',work/(mode+'-native')])
            result=command('run-'+mode,[work/(mode+'-native'),work/'cases.bin',work/(mode+'.bin')]);blob=(work/(mode+'.bin')).read_bytes()
            expected=record['numeric_f32_per_mode']*4+record['numeric_f64_per_mode']*8+128
            assert len(blob)==expected,(mode,len(blob),expected)
            outputs.append(blob);record['modes'].append({'mode':mode,'coax_profile':profile,'passed':True,'numeric_observations':record['numeric_f32_per_mode']+record['numeric_f64_per_mode'],'output_bytes':len(blob),'output_sha256':sha(work/(mode+'.bin')),'native_object_sha256':sha(obj),'stderr_bytes':0,'stdout':result.stdout.decode().strip()})
        assert outputs[0]==outputs[1]==outputs[2]
        for profile,flags in [('debug',['-O0','-g']),('release',['-O2'])]:
            directory=profiles[profile]['path'];raw_obj=work/(profile+'-raw.o')
            command('compile-'+profile+'-raw-object',[cc,*base,*flags,'-c',directory/'raw.c','-o',raw_obj])
            typed=macho_functions(work/(profile+'-typed.o'),names);raw=macho_functions(raw_obj,names)
            record['object_comparison'][profile]={'typed':typed,'raw':raw,'functions':{}}
            if typed['available'] and raw['available']:
                for name in names:
                    a=typed['functions'][name];b=raw['functions'][name]
                    record['object_comparison'][profile]['functions'][name]={'available':a['available'] and b['available'],'symbol_bytes_equal':a.get('hex')==b.get('hex'),'both_relocation_free':a.get('relocation_count')==b.get('relocation_count')==0,'typed_bytes':a.get('bytes'),'raw_bytes':b.get('bytes'),'exact_relocation_free_instruction_extent':a['available'] and b['available'] and a['hex']==b['hex'] and a['relocation_count']==b['relocation_count']==0}
        record['abi']={'three_raw_fields':True,'point_f32_size_alignment_offsets':[12,4,0,4,8],'point_f64_size_alignment_offsets':[24,8,0,8,16],'vectors_and_second_unit_vectors_match_raw_fields':True,'function_pointer_aggregate_params_and_return':True,'named_nested_and_fixed_array_size_alignment_offsets_roundtrip':True,'typed_view_layout_and_operations':True,'whole_aggregate_order_cases_per_carrier':10,'interrupted_foreign_scalar_order_cases_per_carrier':4,'left_view_alias_snapshot_per_carrier':True,'no_foreign_aggregate_admission':True}
        assert {str(p.relative_to(ROOT)):sha(p) for p in source_paths}==source_hashes,'source changed during proof'
        assert sha(COMPILER)==record['compiler_sha256'],'compiler changed during proof'
        record['passed']=True
    finally:
        record['duration_seconds']=time.monotonic()-began;save()
    print(json.dumps({'passed':True,'record':str(work/'receipt.json'),'numeric_observations_per_mode':record['numeric_f32_per_mode']+record['numeric_f64_per_mode'],'duration_seconds':record['duration_seconds']}))
if __name__=='__main__':main()
