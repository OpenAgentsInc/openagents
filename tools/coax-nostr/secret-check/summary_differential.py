import json, pathlib, sys, tempfile
root=pathlib.Path(sys.argv[1]); sys.path.insert(0,str(root/'tools/coax-nostr/secret-check'))
from aggregate import AggregateAudit
cases={
 'public_then_secret':'helper :: (v:u64)->u64 {return ifx v==0 then 1 else 2;} candidate :: (x:u64)->u64 {a:=helper(0);return helper(x)+a;}',
 'dirty_copy':'Pair :: struct {value:u64;} unsafe_vartime :: (x:u64)->u64 {return x;} helper :: (p:Pair,x:u64)->u64 {if p.value==0 {return x;}else {return unsafe_vartime(x);}} candidate :: (x:u64)->u64 {p:=Pair.{value=0};a:=helper(p,x);p.value=1;return helper(p,a);}',
 'repeated_pure':'helper :: (x:u64)->u64 {return x|1;} candidate :: (x:u64)->u64 {a:=helper(x);b:=helper(x);return a^b;}',
 'guard':'helper :: (x:u64)->u64 {#assert x!=0;return x|1;} candidate :: (x:u64)->u64 {a:=helper(x);b:=helper(x);return a^b;}',
 'view_write':'write :: (v:[]u64,x:u64)->bool {v[0]=x;return true;} candidate :: (x:u64)->u64 {a:[2]u64;v:=a[..];ok:=write(v,x);return ifx a[0]==0 then 1 else 2;}',
 'zero_loop':'unsafe_vartime :: (x:u64)->u64 {return x+1;} candidate :: (x:u64)->u64 {flag:u64=1;for i:0..<0 {flag=0;}if flag==0 {return x;}else {return unsafe_vartime(x);}}',
 'vartime':'unsafe_vartime :: (x:u64)->u64 {return x;} helper :: (x:u64)->u64 {return unsafe_vartime(x);} candidate :: (x:u64)->u64 {return helper(x);}' }
out={}; tool=AggregateAudit()
with tempfile.TemporaryDirectory() as t:
 for name,source in cases.items():
  path=pathlib.Path(t)/(name+'.coax');path.write_text(source);r=tool.audit(path,'candidate')
  guards=[(g['line'],g['column'],g['origin_mask'],g['kind']) for g in r['authored_guard_obligations']]
  out[name]={'error':r['error_name'],'accepted':r['accepted'],'return_mask':r['return_dependency_mask'],'reached':r['reached_procedures'],'guards':guards,'visits':r['visited_nodes']}
tool.close();print(json.dumps(out,sort_keys=True,indent=2))
