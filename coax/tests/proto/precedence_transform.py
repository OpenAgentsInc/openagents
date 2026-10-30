"""Descriptor-checked exact old-to-new ordinal error transformation."""
import re
reader=re.compile(r'checked:=([A-Za-z_][A-Za-z0-9_]*)_decode\(msg\.wire\);if msg\.error!=0 \|\| checked\.error!=0 \{return ([A-Za-z_][A-Za-z0-9_]*)\.\{error=msg\.error\|checked\.error(,done=true)?\};\}')
child=re.compile(r'child:=([A-Za-z_][A-Za-z0-9_]*)_decode\(value\.wire\);if value\.error!=0 \|\| child\.error!=0 \{return ([A-Za-z_][A-Za-z0-9_]*)\.\{error=value\.error\|child\.error\};\}')
def read_fix(m):
 dec,typ,shape=m.group(1),m.group(2),m.group(3) or ''
 return f'if msg.error!=0 {{return {typ}.{{error=msg.error{shape}}};}}checked:={dec}_decode(msg.wire);if checked.error!=0 {{return {typ}.{{error=checked.error{shape}}};}}'
def child_fix(m):
 dec,typ=m.groups()
 return f'if value.error!=0 {{return {typ}.{{error=value.error}};}}child:={dec}_decode(value.wire);if child.error!=0 {{return {typ}.{{error=child.error}};}}'
def transform(a,types):
 mapping=re.findall(r'^// (\.[^\n ]+) -> ([A-Za-z0-9_]+)$',a,re.M);assert mapping
 expected_read=expected_child=0
 for full,name in mapping:
  t=types[full]; fields=t.get('field',[]);groups=set()
  for f in fields:
   if 'oneof_index' in f and f.get('proto3_optional',['false'])[0]!='true':groups.add(f['oneof_index'][0])
   if f['type'][0]=='TYPE_MESSAGE':expected_child+=1
  expected_read+=1+len(fields)+len(groups) # one unknown, every field, each real oneof
 b,n=reader.subn(read_fix,a);b,k=child.subn(child_fix,b)
 if n!=expected_read or k!=expected_child:raise ValueError(f'descriptor mismatch: readers {n}/{expected_read}, children {k}/{expected_child}')
 if 'msg.error|checked.error' in b or 'value.error|child.error' in b:raise ValueError('unconverted ordinal combination')
 return b
