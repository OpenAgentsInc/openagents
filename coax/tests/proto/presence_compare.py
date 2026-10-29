#!/usr/bin/env python3
"""Compare two compiler builds using independently decoded descriptor policy.

Usage: presence_compare.py OLD_COMPILER NEW_COMPILER [PROTOC_INCLUDE]
This is a bounded compatibility check, not a full mutation or security audit.
"""
import json,re,subprocess,hashlib,pathlib,sys,shutil
if not __debug__:raise SystemExit('checks require Python assertions')
if len(sys.argv) not in (3,4):raise SystemExit(__doc__)
R=pathlib.Path(__file__).resolve().parents[3];old=sys.argv[1];new=sys.argv[2];include=sys.argv[3] if len(sys.argv)==4 else '/opt/homebrew/include'
print('old compiler',hashlib.sha256(pathlib.Path(old).read_bytes()).hexdigest())
print('new compiler',hashlib.sha256(pathlib.Path(new).read_bytes()).hexdigest())
protoc=shutil.which('protoc')
if not protoc:raise SystemExit('protoc is required for the independent descriptor comparison')
print('protoc',hashlib.sha256(pathlib.Path(protoc).read_bytes()).hexdigest(),subprocess.check_output([protoc,'--version'],text=True).strip())
baseline=json.loads((R/'coax/tests/proto/import-baselines.json').read_text())
def cmd(*args,input=None):return subprocess.run([str(a) for a in args],input=input,capture_output=True,check=True).stdout
def parse_descriptor(data):
 text=cmd(protoc,'-I'+include,'-I',R/'proto','--decode=google.protobuf.FileDescriptorSet','google/protobuf/descriptor.proto','openagents/options/v1alpha1/options.proto',input=data).decode()
 root={};stack=[root]
 for raw in text.splitlines():
  line=raw.strip()
  if line=='}':stack.pop();continue
  if line.endswith(' {'):
   key=line[:-2];node={};stack[-1].setdefault(key,[]).append(node);stack.append(node);continue
  key,sep,value=line.partition(': ')
  if not sep: raise ValueError('unexpected descriptor line '+line)
  if value.startswith('"'):value=json.loads(value)
  stack[-1].setdefault(key,[]).append(value)
 assert len(stack)==1
 types={}
 def add(m,prefix):
  full=prefix+'.'+m['name'][0];types[full]=m
  for child in m.get('nested_type',[]):add(child,full)
 for f in root.get('file',[]):
  pkg=f.get('package',[''])[0];prefix='.'+pkg if pkg else ''
  for m in f.get('message_type',[]):add(m,prefix)
 return types
check=re.compile(r'([A-Za-z0-9_]+)__check :: \(wire:\[\] u8,depth:int,budget:int,required:bool,canonical:bool\)->[A-Za-z0-9_]+Check \{.*?return [A-Za-z0-9_]+Check\.\{left=left\};\}',re.S)
def exactly(text,old,new,count,context):
 actual=text.count(old)
 if actual!=count: raise ValueError(f'{context}: expected {count} occurrences, got {actual}')
 return text.replace(old,new)
def transform(body,types):
 mapping=dict((name,full) for full,name in re.findall(r'^// (\.[^\n ]+) -> ([A-Za-z0-9_]+)$',body,re.M))
 def convert(m):
  name=m.group(1);full=mapping.get(name)
  if full not in types:raise ValueError('missing independent descriptor '+str(full))
  t=m.group(0)
  for f in types[full].get('field',[]):
   name=f['name'][0];label=f['label'][0];opts=f.get('options',[{}])[0]
   required=opts.get('[openagents.options.v1alpha1.required_value]',['false'])[0]=='true'
   maximum=opts.get('[openagents.options.v1alpha1.max_count]',['0'])[0]!='0'
   real='oneof_index' in f and f.get('proto3_optional',['false'])[0]!='true'
   prefix='N__pb_';decl=f'count_{name}:int=0;\n';update=f'count_{name}+=1;\n'
   if label=='LABEL_REPEATED':
    if not (required or maximum):
     t=exactly(t,decl,'',1,full+'.'+name+' decl')
     kind=f['type'][0];count=1 if kind in ('TYPE_STRING','TYPE_BYTES','TYPE_MESSAGE') else 2
     t=exactly(t,update,'',count,full+'.'+name+' count')
    continue
   duplicate=f'if count_{name}>0 {{return {prefix}Check.{{error={prefix}DUPLICATE}};}}\n'
   if real:
    group=f['oneof_index'][0];number=f['number'][0]
    old_group=f'if one_{group}!=0 {{return {prefix}Check.{{error={prefix}DUPLICATE}};}} one_{group}={number};'
    new_group=f'if one_{group} {{return {prefix}Check.{{error={prefix}DUPLICATE}};}} one_{group}=true;'
    if duplicate+old_group not in t:raise ValueError(full+'.'+name+' oneof ordering mismatch')
    t=exactly(t,duplicate,'',1,full+'.'+name+' duplicate')
    t=exactly(t,old_group,new_group,1,full+'.'+name+' group')
    if required:
     t=exactly(t,decl,f'seen_{name}:bool=false;\n',1,full+'.'+name+' decl')
     t=exactly(t,update,f'seen_{name}=true;\n',1,full+'.'+name+' update')
     t=exactly(t,f'required && count_{name}==0',f'required && !seen_{name}',1,full+'.'+name+' required')
    else:
     t=exactly(t,decl,'',1,full+'.'+name+' decl')
     t=exactly(t,update,'',1,full+'.'+name+' update')
   else:
    t=exactly(t,decl,f'seen_{name}:bool=false;\n',1,full+'.'+name+' decl')
    t=exactly(t,duplicate,f'if seen_{name} {{return {prefix}Check.{{error={prefix}DUPLICATE}};}}\n',1,full+'.'+name+' duplicate')
    t=exactly(t,update,f'seen_{name}=true;\n',1,full+'.'+name+' update')
    if required:t=exactly(t,f'required && count_{name}==0',f'required && !seen_{name}',1,full+'.'+name+' required')
  t=re.sub(r'one_(\d+):u32=0;',r'one_\1:bool=false;',t)
  return t
 result=check.sub(convert,body)
 result=result.replace('depth+1,left,true,canonical)', 'depth+1,left,N__pb_require_fields(),canonical)')
 result=result.replace('N__pb_root_depth(),N__pb_default_fields(),true,', 'N__pb_root_depth(),N__pb_default_fields(),N__pb_require_fields(),')
 result=result.replace('N__pb_root_depth(),N__pb_default_fields(),false,', 'N__pb_root_depth(),N__pb_default_fields(),N__pb_partial_fields(),')
 getter=re.compile(r'pos:=0;while pos<msg\.wire\.count \{f:=([A-Za-z0-9_]+)scan\(msg\.wire,pos\);pos=f\.next;if f\.number==(\d+) \{return ([A-Za-z0-9_]+)\.\{value=(.*?),present=true\};\}\}\nreturn \3\.\{\};\}',re.S)
 return getter.sub(r'f:=\1find(msg.wire,\2);return \3.{value=\4,present=!f.done,error=f.error};}',result)
for name,entry in baseline['profiles'].items():
 path=R/entry.get('descriptor_fixture',name);data=path.read_bytes(); assert hashlib.sha256(data).hexdigest()==entry['descriptor_sha256'];args=['proto',path,'--prefix','N']
 a=cmd(old,*args).decode().split('// @proto-descriptor-declarations\n',1)[1];b=cmd(new,*args).decode().split('// @proto-descriptor-declarations\n',1)[1]
 admission='if !N__pb_message_admitted(depth,wire.count) {return N__pb_Check.{error=N__pb_LIMIT};}'
 original='if depth>N__pb_MAX_DEPTH || wire.count<0 || wire.count>N__pb_MAX_BYTES {return N__pb_Check.{error=N__pb_LIMIT};}'
 if a.count(admission)!=len(check.findall(a)) or original in a:raise ValueError(name+' old compiler admission mismatch')
 historic=a.replace(admission,original).replace('N__pb_root_depth(),N__pb_default_fields()','0,N__pb_MAX_FIELDS')
 if hashlib.sha256(historic.encode()).hexdigest()!=entry['declarations_sha256']:raise ValueError(name+' historical fingerprint mismatch')
 expected=transform(a,parse_descriptor(data))
 if expected!=b:
  import difflib
  print('\n'.join(list(difflib.unified_diff(expected.splitlines(),b.splitlines()))[:35]));raise ValueError(name)
 if name=='coax/tests/proto/types.pb':
  tests=[]
  tests.append(('wrong field',a.replace('count_numbers:int=0;','count_wrong:int=0;',1)))
  tests.append(('missing packed increment',a.replace('count_numbers+=1;\n','',1)))
  needle='if count_title>0 {return N__pb_Check.{error=N__pb_DUPLICATE};}\nif one_0!=0 {return N__pb_Check.{error=N__pb_DUPLICATE};} one_0=20;'
  assert needle in a
  tests.append(('duplicate order',a.replace(needle,'if one_0!=0 {return N__pb_Check.{error=N__pb_DUPLICATE};} one_0=20;\nif count_title>0 {return N__pb_Check.{error=N__pb_DUPLICATE};}',1)))
  needle='if f.wt!=5 { return N__pb_Check.{error=N__pb_WIRE}; }';assert needle in a
  tests.append(('wire constraint',a.replace(needle,needle.replace('!=5','!=1'),1)))
  for label,changed in tests:
   try: matched=transform(changed,parse_descriptor(data))==b
   except ValueError:matched=False
   if matched: raise AssertionError('negative control accepted: '+label)
  print('four structural negative controls rejected')
 print(name,'matched')
print(len(baseline['profiles']),'descriptor profiles matched exact output')
