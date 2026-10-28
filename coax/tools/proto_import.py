#!/usr/bin/env python3
"""FileDescriptorSet -> inspectable Coax typed wire views and bounded codecs.
Standard library only. No protoc, network, schema execution or target runtime
Python dependency. This first importer declines maps/proto2/groups/extensions
and duplicate singular/oneof fields rather than guessing their semantics.
"""
import argparse
import hashlib
from collections import Counter
from dataclasses import dataclass, field
from pathlib import Path
import re
import sys

MAX_DESCRIPTOR=8*1024*1024
IDENT=re.compile(r'[A-Za-z_][A-Za-z0-9_]*\Z')


def fields(data):
    out=[];at=0
    def var():
        nonlocal at
        value=0
        for shift in range(0,70,7):
            if at>=len(data):raise ValueError('truncated descriptor varint')
            b=data[at];at+=1
            if shift==63 and b>1:raise ValueError('descriptor varint overflow')
            value|=(b&127)<<shift
            if b<128:return value
        raise ValueError('descriptor varint overflow')
    while at<len(data):
        key=var();num=key>>3;wt=key&7
        if not num or num>536870911:raise ValueError('invalid descriptor field number')
        if wt==0:value=var()
        elif wt in (1,5):
            n=8 if wt==1 else 4
            if n>len(data)-at:raise ValueError('truncated fixed descriptor field')
            value=data[at:at+n];at+=n
        elif wt==2:
            n=var()
            if n>len(data)-at:raise ValueError('truncated descriptor length')
            value=data[at:at+n];at+=n
        else:raise ValueError('descriptor groups are unsupported')
        out.append((num,wt,value))
        if len(out)>40000:raise ValueError('descriptor field budget')
    return out


def one(items,n,default=None):
    values=[v for k,_,v in items if k==n]
    if len(values)>1:raise ValueError('duplicate descriptor metadata field')
    return values[0] if values else default


def many(items,n):return [v for k,_,v in items if k==n]

def string(v):
    if not isinstance(v,bytes):raise ValueError('descriptor string type')
    return v.decode('utf-8')

def ident(v):
    s=string(v)
    if not IDENT.fullmatch(s):raise ValueError('invalid descriptor identifier')
    return s


def options(raw,scope):
    if raw is None:return {}
    opts={}
    allowed={51000,51001,51006} if scope=='message' else {51002,51003,51004,51005,51007,51008}
    for n,w,v in fields(raw):
        if (n==999 or n>=1000) and n not in allowed:raise ValueError('unsupported policy option '+str(n))
        if n in opts:raise ValueError('duplicate descriptor option')
        if n in {51000,51001,51002,51003,51004,51008}:
            if w!=0 or not isinstance(v,int) or v>2**63-1:raise ValueError('invalid numeric policy option')
            if n==51004 and v not in (0,1):raise ValueError('invalid required_value option')
        if n in {51005,51006,51007}:
            if w!=2:raise ValueError('invalid text policy option')
            string(v)
        opts[n]=v
    return opts


@dataclass
class F:
    name:str;number:int;label:int;kind:int;type_name:str;opts:dict;oneof:object=None;optional:bool=False

@dataclass
class M:
    full:str;fs:list;groups:list;opts:dict;name:str=''

@dataclass
class E:
    full:str;values:list;name:str=''


def descriptor(raw,prefix):
    if not IDENT.fullmatch(prefix):raise ValueError('invalid import prefix')
    files=fields(raw);messages=[];enums=[];names=set();file_names=set();dependencies=[]
    def enum(data,parent):
        p=fields(data);full=parent+'.'+ident(one(p,1));values=[]
        for v in many(p,2):
            q=fields(v);n=one(q,2)
            if not isinstance(n,int):raise ValueError('missing enum number')
            if not (n<=2**31-1 or n>=2**64-2**31):raise ValueError('enum value outside int32')
            n=(n&0xffffffff);n=n-2**32 if n>=2**31 else n
            values.append((ident(one(q,1)),n))
        if not values or values[0][1]!=0:raise ValueError('proto3 enum must start at zero')
        if len(enums)>=256 or len(values)>8192 or sum(len(e.values) for e in enums)+len(values)>8192:raise ValueError('descriptor enum budget exceeded')
        enums.append(E(full,values))
    def message(data,parent,depth=0):
        if depth>=32:raise ValueError('descriptor nesting budget')
        p=fields(data);full=parent+'.'+ident(one(p,1));opts=options(one(p,7),'message')
        if opts.get(7):raise ValueError('maps are not supported: '+full)
        if many(p,5) or many(p,6):raise ValueError('message extensions are not supported: '+full)
        groups=[ident(one(fields(g),1)) for g in many(p,8)];fs=[];numbers=set();field_names=set()
        for item in many(p,2):
            q=fields(item);name=ident(one(q,1));number=one(q,3);label=one(q,4);kind=one(q,5)
            if not isinstance(number,int) or not 1<=number<=536870911 or 19000<=number<=19999 or number in numbers or name in field_names:raise ValueError('invalid/duplicate field in '+full)
            if label not in (1,3) or kind not in range(1,19) or kind==10 or one(q,2) or one(q,7) is not None:raise ValueError('unsupported field shape: '+full+'.'+name)
            numbers.add(number);field_names.add(name);o=options(one(q,8),'field')
            if 51007 in o and string(o[51007])!='nonempty-no-nul':raise ValueError('unsupported grammar option in '+full+'.'+name)
            group=one(q,9);optional=bool(one(q,17,0))
            if group is not None and (not isinstance(group,int) or group>=len(groups) or label==3):raise ValueError('invalid oneof')
            if optional and group is None:raise ValueError('proto3 optional lacks synthetic oneof')
            fs.append(F(name,number,label,kind,string(one(q,6,b'')),o,group,optional))
        if len(fs)>256:raise ValueError('per-message field budget exceeded')
        if len(messages)>=256:raise ValueError('descriptor message budget exceeded')
        messages.append(M(full,fs,groups,opts))
        for n in many(p,3):message(n,full,depth+1)
        for n in many(p,4):enum(n,full)
    for data in many(files,1):
        p=fields(data);name=string(one(p,1));package=string(one(p,2,b''))
        if name in file_names:raise ValueError('duplicate descriptor file')
        if len(file_names)>=256:raise ValueError('descriptor file budget exceeded')
        file_names.add(name);dependencies.extend(string(v) for v in many(p,3))
        # These files describe annotations, not target data types.
        if name in ('google/protobuf/descriptor.proto','openagents/options/v1alpha1/options.proto'):continue
        if one(p,12,b'')!=b'proto3' or many(p,7) or one(p,14) is not None:raise ValueError('only proto3 data descriptors are supported: '+name)
        if package and any(not IDENT.fullmatch(part) for part in package.split('.')):raise ValueError('invalid package')
        parent='.'+package if package else ''
        for m in many(p,4):message(m,parent)
        for e in many(p,5):enum(e,parent)
    if any(d not in file_names for d in dependencies):raise ValueError('missing descriptor file dependency')
    if not messages:raise ValueError('no runtime messages in descriptor')
    if len(messages)>256 or sum(len(m.fs) for m in messages)>2048:raise ValueError('descriptor type/field budget exceeded')
    all_types=messages+enums;counts=Counter(v.full.rsplit('.',1)[-1] for v in all_types)
    mapping={}
    for v in all_types:
        if v.full in mapping:raise ValueError('duplicate fully-qualified type')
        leaf=v.full.rsplit('.',1)[-1]
        v.name=prefix+'_'+(leaf if counts[leaf]==1 else v.full.lstrip('.').replace('.','_'))
        if v.name in names:raise ValueError('generated name collision; use narrower profiles')
        names.add(v.name);mapping[v.full]=v
    for m in messages:
        for f in m.fs:
            if f.kind in (11,14) and f.type_name not in mapping:raise ValueError('missing descriptor dependency: '+f.type_name)
            if f.kind==11 and not isinstance(mapping[f.type_name],M):raise ValueError('message type mismatch')
            if f.kind==14 and not isinstance(mapping[f.type_name],E):raise ValueError('enum type mismatch')
            if f.kind not in (11,14) and f.type_name:raise ValueError('scalar has type_name')
            if f.opts.get(51002) and f.kind not in (9,12):raise ValueError('max_bytes requires bytes/string')
            if f.opts.get(51008) and f.kind not in (9,12):raise ValueError('exact_bytes requires bytes/string')
            if f.opts.get(51003) and f.label!=3:raise ValueError('max_count requires repeated field')
            if f.opts.get(51007) and f.kind!=9:raise ValueError('grammar requires string')
            if f.optional and sum(x.oneof==f.oneof for x in m.fs)!=1:raise ValueError('invalid synthetic oneof')
    return messages,enums,mapping


# protobuf descriptor type -> Coax scalar, option wrapper, wire type
SCALAR={1:('f64','F64',1),2:('f32','F32',5),3:('s64','S64',0),4:('u64','U64',0),5:('s32','S32',0),6:('u64','U64',1),7:('u32','U32',5),8:('bool','Bool',0),9:('string','String',2),12:('[] u8','Bytes',2),13:('u32','U32',0),14:('s32','S32',0),15:('s32','S32',5),16:('s64','S64',1),17:('s32','S32',0),18:('s64','S64',0)}

class Generator:
    def __init__(self,messages,enums,mapping,prefix):
        self.ms=messages;self.es=enums;self.mapping=mapping;self.p=prefix+'__pb_';self.lines=[]
    def emit(self,s=''):self.lines.append(s)
    def typ(self,f):return self.mapping[f.type_name].name if f.kind==11 else SCALAR[f.kind][0]
    def option(self,f):return self.typ(f)+'_Option' if f.kind==11 else self.p+SCALAR[f.kind][1]
    def wt(self,f):return 2 if f.kind==11 else SCALAR[f.kind][2]
    def value(self,f,raw='f'):
        r=raw+'.raw';data=raw+'.payload'
        if f.kind==11:return self.typ(f)+'.{wire='+data+'}'
        if f.kind==9:return 'cast(string) '+data
        if f.kind==12:return data
        if f.kind in (3,5,14,15,16):return 'cast,trunc('+self.typ(f)+') '+r
        if f.kind in (4,6):return r
        if f.kind in (7,13):return 'cast(u32) '+r
        if f.kind==8:return r+'!=0'
        if f.kind in (17,18):return 'cast('+self.typ(f)+') '+self.p+'sint('+r+')'
        if f.kind==1:return 'f64_from_bits('+r+')'
        if f.kind==2:return 'f32_from_bits(cast(u32) '+r+')'
        raise ValueError('unimplemented scalar')
    def constraints(self,f,fail):
        p=self.p;self.emit('if f.wt!='+str(self.wt(f))+' { '+fail(p+'WIRE')+' }')
        if f.kind in (7,13,17):self.emit('if f.raw>4294967295 { '+fail(p+'VALUE')+' }')
        if f.kind==8:self.emit('if canonical && f.raw>1 { '+fail(p+'VALUE')+' }')
        if f.kind==9:self.emit('if !'+p+'utf8(f.payload) { '+fail(p+'UTF8')+' }')
        if f.kind in (9,12):
            for opt,op in [(51002,'>'),(51008,'!=')]:
                if f.opts.get(opt):self.emit('if f.payload.count'+op+str(f.opts[opt])+' { '+fail(p+'LIMIT')+' }')
            if f.opts.get(51007):self.emit('if !'+p+'no_nul(f.payload) { '+fail(p+'VALUE')+' }')
        if f.kind in (1,2) and f.opts.get(51005)==b'rad':
            x=self.value(f);self.emit('x:='+x+';if x!=x || x < -3.141592653589793 || x>3.141592653589793 { '+fail(p+'VALUE')+' }')
    def check(self,m):
        n=m.name;p=self.p
        self.emit(f'{n}__check :: (wire:[] u8,depth:int,budget:int,required:bool,canonical:bool)->{p}Check {{')
        self.emit(f'if depth>{p}MAX_DEPTH || wire.count<0 || wire.count>{p}MAX_BYTES {{return {p}Check.{{error={p}LIMIT}};}}')
        if m.opts.get(51001):self.emit(f'if wire.count>{m.opts[51001]} {{return {p}Check.{{error={p}LIMIT}};}}')
        self.emit('left:=budget;pos:=0;')
        for f in m.fs:self.emit(f'count_{f.name}:int=0;')
        real_groups={f.oneof for f in m.fs if f.oneof is not None and not f.optional}
        for group in sorted(real_groups):self.emit(f'one_{group}:u32=0;')
        self.emit('while pos<wire.count {')
        self.emit(f'if left<=0 {{return {p}Check.{{error={p}LIMIT}};}} left-=1;')
        self.emit(f'f:={p}scan(wire,pos);if f.error!=0 {{return {p}Check.{{error=f.error}};}} if canonical && !{p}minimal(wire,f) {{return {p}Check.{{error={p}WIRE}};}} pos=f.next;')
        fail=lambda error:f'return {p}Check.{{error={error}}};'
        for index,f in enumerate(m.fs):
            self.emit(('if' if index==0 else 'else if')+f' f.number=={f.number} {{')
            if f.label!=3:self.emit(f'if count_{f.name}>0 {{'+fail(p+'DUPLICATE')+'}')
            if f.oneof in real_groups and not f.optional:self.emit(f'if one_{f.oneof}!=0 {{'+fail(p+'DUPLICATE')+f'}} one_{f.oneof}={f.number};')
            pack=f.label==3 and self.wt(f)!=2
            if pack:
                self.emit('if f.wt==2 {packed:=f.payload;at:=0;while at<packed.count {')
                self.emit(f'if left<=0 {{'+fail(p+'LIMIT')+f'}} left-=1; f={p}scalar(packed,at,{self.wt(f)});if f.error!=0 {{'+fail('f.error')+'}')
                if self.wt(f)==0:self.emit(f'if canonical && {p}vsize(f.raw)!=f.next-at {{'+fail(p+'WIRE')+'}')
                self.emit('at=f.next;')
                self.emit(f'f.wt={self.wt(f)};')
                self.constraints(f,fail);self.emit(f'count_{f.name}+=1;')
                self.emit('}} else {');self.constraints(f,fail);self.emit(f'count_{f.name}+=1;');self.emit('}')
            else:
                self.constraints(f,fail);self.emit(f'count_{f.name}+=1;')
                if f.kind==11:
                    self.emit(f'child:={self.typ(f)}__check(f.payload,depth+1,left,true,canonical);if child.error!=0 {{'+fail('child.error')+'} left=child.left;')
            if f.opts.get(51003):self.emit(f'if count_{f.name}>{f.opts[51003]} {{'+fail(p+'LIMIT')+'}')
            self.emit('}')
        self.emit('}')
        for f in m.fs:
            if f.opts.get(51004):self.emit(f'if required && count_{f.name}==0 {{'+fail(p+'REQUIRED')+'}')
        self.emit(f'return {p}Check.{{left=left}};}}')
    def api(self,m):
        n=m.name;p=self.p
        self.emit(f'{n}_new :: ()->{n} {{return {n}.{{}};}}')
        self.emit(f'{n}_decode :: (wire:[] u8)->{n} {{c:={n}__check(wire,0,{p}MAX_FIELDS,true,false);if c.error!=0 {{return {n}.{{error=c.error}};}}return {n}.{{wire=wire}};}}')
        self.emit(f'{n}_decode_minimal :: (wire:[] u8)->{n} {{c:={n}__check(wire,0,{p}MAX_FIELDS,true,true);if c.error!=0 {{return {n}.{{error=c.error}};}}return {n}.{{wire=wire}};}}')
        self.emit(f'{n}_encode :: (msg:{n},out:[] u8)->{p}View {{if msg.error!=0 {{return {p}View.{{error=msg.error}};}} c:={n}__check(msg.wire,0,{p}MAX_FIELDS,true,false);if c.error!=0 {{return {p}View.{{error=c.error}};}} return {p}copy(msg.wire,out);}}')
        for group in sorted({f.oneof for f in m.fs if f.oneof is not None and not f.optional}):
            group_name=m.groups[group];targets=[f'f.number=={f.number}' for f in m.fs if f.oneof==group]
            self.emit(f'{n}_case_{group_name} :: (msg:{n})->{p}U32 {{checked:={n}_decode(msg.wire);if msg.error!=0 || checked.error!=0 {{return {p}U32.{{error=msg.error|checked.error}};}}pos:=0;while pos<msg.wire.count {{f:={p}scan(msg.wire,pos);pos=f.next;if '+ ' || '.join(targets)+f' {{return {p}U32.{{value=f.number,present=true}};}}}} return {p}U32.{{}};}}')
        tests=' || '.join('f.number=='+str(f.number) for f in m.fs) or 'false'
        self.emit(f'{n}_unknown :: (msg:{n},cursor:int)->{p}Field {{checked:={n}_decode(msg.wire);if msg.error!=0 || checked.error!=0 {{return {p}Field.{{error=msg.error|checked.error}};}}pos:=cursor;while true {{f:={p}scan(msg.wire,pos);if f.error!=0 || f.done {{return f;}}pos=f.next;if !('+tests+') {return f;}}return '+p+'Field.{done=true};}')
        for f in m.fs:
            self.emit(f'{n}_FIELD_{f.name} :: {f.number};')
            if f.label==3:self.iterator(m,f)
            else:self.getter(m,f)
            self.writer(m,f)
    def getter(self,m,f):
        n=m.name;p=self.p;o=self.option(f)
        self.emit(f'{n}_get_{f.name} :: (msg:{n})->{o} {{')
        self.emit(f'checked:={n}_decode(msg.wire);if msg.error!=0 || checked.error!=0 {{return {o}.{{error=msg.error|checked.error}};}}')
        self.emit(f'pos:=0;while pos<msg.wire.count {{f:={p}scan(msg.wire,pos);pos=f.next;if f.number=={f.number} {{return {o}.{{value='+self.value(f)+',present=true};}}')
        self.emit(f'return {o}.{{}};}}')
    def iterator(self,m,f):
        n=m.name;p=self.p;t=self.typ(f);iterator=n+'_Iter_'+f.name
        self.emit(f'{iterator} :: struct {{wire:[] u8;cursor:int;packed:[] u8;packed_at:int;value:{t};has_value:bool;done:bool;error:u32;}}')
        self.emit(f'{n}_iter_{f.name} :: (msg:{n})->{iterator} {{checked:={n}_decode(msg.wire);if msg.error!=0 || checked.error!=0 {{return {iterator}.{{error=msg.error|checked.error,done=true}};}}return {n}_next_{f.name}({iterator}.{{wire=msg.wire}});}}')
        self.emit(f'{n}_next_{f.name} :: (old:{iterator})->{iterator} {{iterator:=old;iterator.has_value=false;if iterator.error!=0 || iterator.done {{return iterator;}}while true {{')
        if self.wt(f)!=2:
            self.emit(f'if iterator.packed_at<iterator.packed.count {{f:={p}scalar(iterator.packed,iterator.packed_at,{self.wt(f)});if f.error!=0 {{iterator.error=f.error;iterator.done=true;return iterator;}}iterator.packed_at=f.next;iterator.value='+self.value(f)+';iterator.has_value=true;return iterator;}')
        self.emit(f'f:={p}scan(iterator.wire,iterator.cursor);if f.error!=0 || f.done {{iterator.error=f.error;iterator.done=true;return iterator;}}iterator.cursor=f.next;if f.number!={f.number} {{continue;}}')
        if self.wt(f)!=2:self.emit('if f.wt==2 {iterator.packed=f.payload;iterator.packed_at=0;continue;}')
        self.emit(f'iterator.value='+self.value(f)+';iterator.has_value=true;return iterator;}return iterator;}')
    def writer(self,m,f):
        n=m.name;p=self.p;t=self.typ(f);wt=self.wt(f);operation='append' if f.label==3 else 'set'
        siblings=[x.number for x in m.fs if x.oneof==f.oneof and f.oneof is not None and not f.optional] if f.oneof is not None and not f.optional else [f.number]
        for clear in (False,True):
            method='clear' if clear else operation;params=f'msg:{n},'+('' if clear else f'value:{t},')+'out:[] u8'
            self.emit(f'{n}_{method}_{f.name} :: ({params})->{n} {{')
            self.emit(f'if msg.error!=0 {{return {n}.{{error=msg.error}};}} checked:={n}__check(msg.wire,0,{p}MAX_FIELDS,false,false);if checked.error!=0 {{return {n}.{{error=checked.error}};}}')
            targets=[] if not clear and f.label==3 else ([f.number] if clear else siblings)
            if targets:self.emit('removed_fields:= ['+str(len(targets))+'] u32.['+','.join(map(str,targets))+'];')
            else:self.emit('removed_fields:[] u32;')
            remove='removed_fields[..]' if targets else 'removed_fields'
            self.emit('data:[] u8;txt:="";raw:u64=0;is_text:=false;')
            if not clear:
                if f.kind==11:
                    self.emit(f'child:={t}_decode(value.wire);if value.error!=0 || child.error!=0 {{return {n}.{{error=value.error|child.error}};}} data=value.wire;')
                elif f.kind==9:self.emit('txt=value;is_text=true;')
                elif f.kind==12:self.emit('data=value;')
                elif f.kind in (1,2):self.emit('raw=cast(u64) '+('f64_bits' if f.kind==1 else 'f32_bits')+'(value);')
                elif f.kind in (17,18):self.emit(f'raw={p}zigzag(cast(s64)value);')
                elif f.kind==8:self.emit('if value {raw=1;}')
                elif f.kind in (3,5,14,15,16):self.emit('raw=cast,trunc(u64) value;')
                else:self.emit('raw=cast(u64) value;')
            self.emit(f'result:={p}rewrite(msg.wire,{remove},{f.number},{wt},raw,data,txt,is_text,'+('true' if clear else 'false')+',out);')
            self.emit(f'if result.error!=0 {{return {n}.{{error=result.error}};}} final:={n}__check(result.wire,0,{p}MAX_FIELDS,false,false);if final.error!=0 {{return {n}.{{error=final.error}};}}return {n}.{{wire=result.wire}};}}')
    def generate(self):
        self.emit(re.sub(r'\bPB_[A-Za-z0-9_]*\b', lambda m: self.p+m[0][3:], Path(__file__).with_name('proto_runtime.coax').read_text()))
        for e in self.es:
            for name,value in e.values:self.emit(f'{e.name}_{name} :: {value};')
            self.emit(f'{e.name}_known :: (value:s32)->bool {{return '+' || '.join('value=='+str(v) for _,v in e.values)+';}')
        for m in self.ms:
            self.emit('// '+m.full+' -> '+m.name)
            self.emit(f'{m.name} :: struct {{wire:[] u8;error:u32;}}')
            self.emit(f'{m.name}_Option :: struct {{value:{m.name};present:bool;error:u32;}}')
        for m in self.ms:self.check(m);self.api(m)
        result='\n'.join(self.lines)+'\n'
        declared=re.findall(r'^([A-Za-z_][A-Za-z0-9_]*) ::',result,re.M)
        if len(declared)!=len(set(declared)):raise ValueError('generated symbol collision; select a narrower descriptor profile')
        if len(result)>8*1024*1024:raise ValueError('generated source budget')
        return result


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--descriptor',type=Path,required=True);parser.add_argument('--prefix',required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    with args.descriptor.open('rb') as source:raw=source.read(MAX_DESCRIPTOR+1)
    if len(raw)>MAX_DESCRIPTOR:raise ValueError('descriptor byte budget exceeded')
    ms,es,mapping=descriptor(raw,args.prefix)
    runtime=Path(__file__).with_name('proto_runtime.coax')
    provenance='// coax-proto-v1 '+ ' '.join([
        'descriptor_sha256='+hashlib.sha256(raw).hexdigest(),
        'tool_sha256='+hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'runtime_sha256='+hashlib.sha256(runtime.read_bytes()).hexdigest(),
        'python='+sys.version.split()[0], 'prefix='+args.prefix])
    args.output.write_text(provenance+'\n'+Generator(ms,es,mapping,args.prefix).generate())

if __name__=='__main__':
    try:main()
    except (ValueError,OSError,UnicodeError,RecursionError,KeyError,TypeError) as error:
        sys.exit('coax proto: '+str(error))
