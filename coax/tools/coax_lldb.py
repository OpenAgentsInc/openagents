"""Optional LLDB presentation for compiler-owned schema-1 source maps.
Load with `command script import /path/to/coax/tools/coax_lldb.py`.
The compiler and target never import or require this development tool.
"""
import hashlib, json, os, shlex
_maps={}
def load(debugger,command,result,internal):
 try:
  with open(shlex.split(command)[0],encoding='utf-8') as file:data=json.load(file)
  if data.get('schema_version')!=1:raise ValueError('unsupported source map schema')
  for path,digest in data['source_hashes'].items():
   with open(path,'rb') as source:
    if hashlib.sha256(source.read()).hexdigest()!=digest:raise ValueError('source changed since compilation: '+path)
  _maps.update({(f['c_name'],os.path.realpath(f['file'])):f for f in data['functions']})
  result.PutCString('Coax source map loaded')
 except (OSError,ValueError,KeyError,IndexError) as error:result.SetError(str(error))
def metadata(frame):
 entry=frame.GetLineEntry().GetFileSpec()
 path=os.path.realpath(os.path.join(entry.GetDirectory() or "",entry.GetFilename() or ""))
 return _maps.get((frame.GetFunctionName(),path),{})
def variables(debugger,command,result,internal):
 frame=debugger.GetSelectedTarget().GetProcess().GetSelectedThread().GetSelectedFrame()
 meta=metadata(frame)
 for variable in meta.get('variables',[]):
  value=frame.FindVariable(variable['c_name'])
  if value.IsValid() and value.GetError().Success():
   text=value.GetValue() or value.GetSummary() or str(value)
   result.PutCString(f"{variable['name']} = {text} ({variable['type']})")
def backtrace(debugger,command,result,internal):
 thread=debugger.GetSelectedTarget().GetProcess().GetSelectedThread()
 for frame in thread:
  name=frame.GetFunctionName() or '?';meta=metadata(frame)
  if meta:
   entry=frame.GetLineEntry();result.PutCString(f"#{frame.GetFrameID()} {meta['name']} at {entry.GetFileSpec()}:{entry.GetLine()}")
  else:result.PutCString(str(frame))
def __lldb_init_module(debugger,internal):
 debugger.HandleCommand('command script add -f coax_lldb.load coax-map')
 debugger.HandleCommand('command script add -f coax_lldb.variables coax-vars')
 debugger.HandleCommand('command script add -f coax_lldb.backtrace coax-bt')
