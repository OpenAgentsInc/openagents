#!/usr/bin/env python3
"""Explicit deterministic Unicode-data tooling, never a normal-build dependency.

No host Unicode-normalization API or downloaded program is used. Committed
Coax tables derive only from the original hash-pinned Unicode 18.0.0 payloads.
"""
import argparse,gzip,hashlib,json,re
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
CORPUS=ROOT/'coax/tests/unicode/upstream/unicode-18.0.0'
TARGET=ROOT/'coax/lib/text/nfkc-data.coax'
VERSION='18.0.0'
def digest(data):return hashlib.sha256(data).hexdigest()
def read_pinned(folder=CORPUS):
    manifest=json.loads((folder/'SOURCES.json').read_text())
    if manifest['schema']!='coax.unicode-source-pins.v1' or manifest['unicode_version']!=VERSION or manifest['normalization_annex_revision']!=58:raise ValueError('Unicode source identity')
    result={};seen=set()
    for row in manifest['files']:
        name=row['file']
        if name in seen or Path(name).name!=name:raise ValueError('duplicate or nonlocal source')
        seen.add(name);data=(folder/name).read_bytes()
        if len(data)!=row['bytes'] or digest(data)!=row['sha256']:raise ValueError('source checksum: '+name)
        if row.get('encoding')=='gzip':
            data=gzip.decompress(data);name=name.removesuffix('.gz')
            if len(data)!=row['source_bytes'] or digest(data)!=row['source_sha256']:raise ValueError('original payload checksum: '+name)
        elif row.get('encoding')is not None:raise ValueError('unsupported source container')
        result[name]=data.decode('utf-8')
    if set(p.name for p in folder.iterdir() if p.is_file())!=seen|{'SOURCES.json'}:raise ValueError('unlisted or missing source file')
    if 'final data files for version 18.0.0'not in result['ReadMe.txt']:raise ValueError('UCD version')
    if '# NormalizationTest-18.0.0.txt'not in result['NormalizationTest.txt']:raise ValueError('normalization corpus version')
    return result
def code_range(text):
    parts=text.strip().split('..');start=int(parts[0],16);end=int(parts[-1],16)
    if not 0<=start<=end<=0x10ffff:raise ValueError('scalar range')
    return range(start,end+1)
def model(source):
    ccc={};decompositions={};canonical={};assigned=set();pending=None
    for line in source['UnicodeData.txt'].splitlines():
        cells=line.split(';')
        if len(cells)!=15:raise ValueError('UnicodeData row')
        cp=int(cells[0],16);name=cells[1];combining=int(cells[3]);raw=cells[5]
        if not 0<=cp<=0x10ffff or not 0<=combining<=255:raise ValueError('codepoint/class')
        if name.endswith(', First>'):
            if pending is not None:raise ValueError('nested UnicodeData range')
            pending=(cp,combining,cells[2]);continue
        if name.endswith(', Last>'):
            if pending is None or cp<pending[0] or combining!=pending[1] or cells[2]!=pending[2] or raw:raise ValueError('UnicodeData range end')
            for item in range(pending[0],cp+1):
                assigned.add(item)
                if combining:ccc[item]=combining
            pending=None;continue
        if pending is not None or cp in assigned:raise ValueError('UnicodeData ordering/range')
        assigned.add(cp)
        if combining:ccc[cp]=combining
        if raw:
            pieces=raw.split();compat=pieces[0].startswith('<')
            if compat:pieces=pieces[1:]
            values=tuple(int(x,16) for x in pieces)
            if not values or any(x<0 or x>0x10ffff or 0xd800<=x<=0xdfff for x in values):raise ValueError('decomposition scalar')
            decompositions[cp]=values
            if not compat:canonical[cp]=values
    if pending is not None:raise ValueError('unterminated UnicodeData range')
    excluded=set()
    for row in source['DerivedNormalizationProps.txt'].splitlines():
        row=row.split('#',1)[0].strip()
        if not row:continue
        fields=[x.strip() for x in row.split(';')]
        if fields[1]=='Full_Composition_Exclusion':excluded.update(code_range(fields[0]))
    explicit=set()
    for row in source['CompositionExclusions.txt'].splitlines():
        row=row.split('#',1)[0].strip()
        if row:explicit.update(code_range(row))
    if not explicit<=excluded:raise ValueError('composition-exclusion source mismatch')
    expanded={}
    def expand(cp,path=()):
        if cp in path or len(path)>32:raise ValueError('decomposition cycle/depth')
        if 0xac00<=cp<0xac00+11172:
            s=cp-0xac00;l=0x1100+s//588;v=0x1161+(s%588)//28;t=s%28
            return (l,v,0x11a7+t)if t else(l,v)
        if cp in expanded:return expanded[cp]
        if cp not in decompositions:return(cp,)
        result=tuple(item for child in decompositions[cp] for item in expand(child,path+(cp,)))
        if len(result)>18:raise ValueError('decomposition exceeds declared workspace bound')
        expanded[cp]=result;return result
    for cp in decompositions:expand(cp)
    compositions={}
    for cp,values in canonical.items():
        if cp not in excluded and len(values)==2:
            a,b=values
            if ccc.get(a,0)!=0:raise ValueError('composition starter')
            key=(a<<21)|b
            if key in compositions:raise ValueError('duplicate composition pair')
            compositions[key]=cp
    return {'ccc':ccc,'decomposition':expanded,'composition':compositions,'assigned':assigned}
def array(name,type_,values):
    values=list(values);lines=[f'{name} :: [{len(values)}]{type_}.[']
    for i in range(0,len(values),12):lines.append('  '+','.join(str(x)for x in values[i:i+12])+(','if i+12<len(values)else''))
    return '\n'.join(lines+["];",''])
def generate(source):
    m=model(source);keys=sorted(m['decomposition']);offsets=[0];flat=[]
    for cp in keys:flat.extend(m['decomposition'][cp]);offsets.append(len(flat))
    ck=sorted(m['ccc']);pk=sorted(m['composition'])
    text='// Derived Unicode 18.0.0 NFKC data. Regenerate explicitly with tools/coax-unicode/tables.py.\n// Original payloads, URLs, license and hashes: coax/tests/unicode/upstream/unicode-18.0.0/SOURCES.json.\n'
    for name,type_,values in [('UN_CCC_KEYS','u32',ck),('UN_CCC_VALUES','u32',[m['ccc'][cp]for cp in ck]),('UN_DECOMP_KEYS','u32',keys),('UN_DECOMP_OFFSETS','u32',offsets),('UN_DECOMP_VALUES','u32',flat),('UN_COMPOSE_KEYS','u64',pk),('UN_COMPOSE_VALUES','u32',[m['composition'][k]for k in pk])]:text+=array(name,type_,values)
    return text,{'version':VERSION,'combining_classes':len(ck),'decompositions':len(keys),'decomposed_values':len(flat),'composition_pairs':len(pk),'assigned_codepoints':len(m['assigned']),'max_decomposition':max(map(len,m['decomposition'].values()))}
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--corpus',type=Path,default=CORPUS);p.add_argument('--output',type=Path,default=TARGET);p.add_argument('--check',action='store_true');args=p.parse_args()
    text,summary=generate(read_pinned(args.corpus))
    if args.check:
        if args.output.read_text()!=text:raise SystemExit('generated Unicode tables drift; regenerate explicitly')
    else:args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(text)
    print(json.dumps({**summary,'table_sha256':digest(text.encode())},sort_keys=True))
if __name__=='__main__':main()
