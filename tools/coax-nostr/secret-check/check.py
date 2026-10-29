#!/usr/bin/env python3
"""Optional audit I/O driver. All language parsing, flow and admission are Coax."""
import argparse
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax')).resolve()
class String(C.Structure):_fields_=[('data',C.c_void_p),('count',C.c_int64)]
class Type(C.Structure):_fields_=[('unit',C.c_int64),('procedure',String),('procedure_line',C.c_int64),('name',String),('line',C.c_int64),('role',String),('type',String)]
class Result(C.Structure):_fields_=[('error',C.c_uint32),('unit',C.c_int64),('line',C.c_int64),('column',C.c_int64),('reached',C.c_int64),('visits',C.c_int64),('return_mask',C.c_uint64)]
class PathValue(C.Structure):_fields_=[('value',String)]
def view_type(item):
    class View(C.Structure):_fields_=[('data',C.POINTER(item)),('count',C.c_int64)]
    return View
IntView=view_type(C.c_int64);ByteView=view_type(C.c_uint8);PathView=view_type(PathValue)
def run(*args,**kwargs):
    result=subprocess.run([str(x) for x in args],capture_output=True,text=True,**kwargs)
    if result.returncode:raise RuntimeError(f'{args[0]} failed ({result.returncode}): {result.stderr or result.stdout}')
    return result
def metadata_sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def bounded_run_steps(value):
    try:steps=int(value)
    except (TypeError,ValueError):raise argparse.ArgumentTypeError('run steps must be an integer')
    if steps<1 or steps>250000000:raise argparse.ArgumentTypeError('run steps must be between 1 and 250000000')
    return steps

class Audit:
    def __init__(self,compiler=COMPILER,directory=None,_policy_source=None,_bridge_source=None):
        self.compiler=Path(compiler).resolve();self.temporary=tempfile.TemporaryDirectory(prefix='coax-secret-check-') if directory is None else None
        self.directory=Path(self.temporary.name) if self.temporary else Path(directory);self.directory.mkdir(parents=True,exist_ok=True)
        self.compiler_fingerprint=metadata_sha(self.compiler);self.compiler_version=run(self.compiler,'--version').stdout.strip()
        self.policy_source=_policy_source or ROOT/'coax/lib/crypto/secret-flow.coax';self.bridge_source=_bridge_source or HERE/'bridge.cpp'
        compiled_maps=[]
        bridge_bytes=self.bridge_source.read_bytes();bridge_copy=self.directory/'bridge-build.cpp';bridge_copy.write_bytes(bridge_bytes)
        for name,source in [('frontend',HERE/'frontend.coax'),('policy',self.policy_source)]:
            output=self.directory/(name+'.c');map_path=self.directory/(name+'-map.json');output.write_text(run(self.compiler,'c',source,'--library','--header',self.directory/(name+'.h'),'--source-map',map_path).stdout);compiled_maps.append(json.loads(map_path.read_text()))
            run('cc','-std=c11','-O2','-fPIC','-c',output,'-o',self.directory/(name+'.o'))
        self.artifact=self.directory/'secret-check.dylib'
        run('c++','-std=c++20','-O2','-shared','-fPIC','-I'+str(self.directory),self.directory/'frontend.o',self.directory/'policy.o',bridge_copy,'-o',self.artifact)
        self.lib=C.CDLL(str(self.artifact))
        self.lib.SC_source_limit.restype=C.c_int64
        self.lib.SC_units.argtypes=[C.c_int64];self.lib.SC_units.restype=C.c_bool
        self.lib.SC_inputs.argtypes=[IntView,C.c_int64];self.lib.SC_inputs.restype=C.c_bool
        self.lib.SC_identity.argtypes=[String,ByteView];self.lib.SC_identity.restype=C.c_bool
        self.lib.SC_closure.argtypes=[PathView,PathView];self.lib.SC_closure.restype=C.c_bool
        self.lib.SC_error_name.argtypes=[C.c_uint32];self.lib.SC_error_name.restype=String
        self.lib.sha256_hash.argtypes=[ByteView,ByteView];self.lib.sha256_hash.restype=C.c_uint32
        self._bind_bridge()
        # Bind tool inputs to the actual compiled snapshots too. The normal
        # compiler source maps are authoritative; `deps` is not a flow proof.
        self.inputs={}
        for metadata in compiled_maps:
            for path,expected in metadata['source_hashes'].items():
                digest=self.source_hash(Path(path).read_bytes());encoded=C.create_string_buffer(expected.encode())
                if not self.lib.SC_identity(String(C.cast(encoded,C.c_void_p),len(expected)),ByteView(digest,32)):
                    raise ValueError('audit tool source changed during its build')
                self.inputs[path]=expected
        if self.bridge_source.read_bytes()!=bridge_bytes:raise ValueError('audit bridge changed during its build')
        for path in [self.bridge_source,Path(__file__)]:self.inputs[str(path)]=metadata_sha(path)
        if metadata_sha(self.compiler)!=self.compiler_fingerprint:raise ValueError('compiler changed during audit tool build')
    def _bind_bridge(self):
        self.lib.sc_bridge.argtypes=[C.POINTER(String),C.c_int64,C.POINTER(Type),C.c_int64,String];self.lib.sc_bridge.restype=Result
    def _analyze(self,sources,types,entry,compiled_runs=False):
        return self.lib.sc_bridge(sources,len(sources),types,len(types),entry)
    def _decorate_report(self,report):return report
    def close(self):
        if self.temporary:self.temporary.cleanup()
    def source_hash(self,data):
        source=(C.c_uint8*len(data)).from_buffer_copy(data);digest=(C.c_uint8*32)()
        if self.lib.sha256_hash(ByteView(source,len(data)),ByteView(digest,32))!=0:raise ValueError('Coax source hashing refused')
        return digest
    def audit(self,path,entry,after_compile=None,run_steps=None):
        if run_steps is not None:
            if type(run_steps) is not int:raise ValueError('run steps must be an integer')
            try:bounded_run_steps(run_steps)
            except argparse.ArgumentTypeError as error:raise ValueError(str(error)) from error
        path=Path(path).resolve();keep=[]
        def string(value):
            data=value if isinstance(value,bytes) else value.encode();buffer=C.create_string_buffer(data);keep.append(buffer)
            return String(C.cast(buffer,C.c_void_p),len(data))
        def paths(values):
            array=(PathValue*len(values))(*[PathValue(string(x)) for x in values]);keep.append(array);return PathView(array,len(values))
        compiler_hash=self.compiler_fingerprint
        report={'schema':'coax.secret-source-audit.v1','entry':entry,'classification':'all entry parameters secret','profile':'unsigned-scalar-source-v1','accepted':False,
                'compiler_sha256':compiler_hash,'compiler':self.compiler_version,'compiler_run_steps':run_steps,'audit_artifact_sha256':metadata_sha(self.artifact),
                'audit_input_sha256':self.inputs,'object_timing_approved':False,'checked_runtime_guards_proved':False,'aggregate_memory_loops_supported':False}
        def finish(error,**extra):
            name=self.lib.SC_error_name(error);report.update(error=error,error_name=C.string_at(name.data,name.count).decode(),accepted=error==0,**extra);return self._decorate_report(report)
        if metadata_sha(self.compiler)!=self.compiler_fingerprint:return finish(13)
        source_map=self.directory/'source-map.json'
        command=[str(self.compiler),'c',str(path),'--library','--source-map',str(source_map)]
        if run_steps is not None:command.extend(['--run-steps',str(run_steps)])
        checked=subprocess.run(command,capture_output=True,text=True)
        if checked.returncode:return finish(1)
        if after_compile:after_compile()
        # The real typed compiler's source map is its resolved source inventory.
        # Standalone `deps` also requires an executable main on private libraries.
        # Descriptor import syntax is refused by the unmodified parser below.
        try:
            map_bytes=source_map.read_bytes();metadata=json.loads(map_bytes)
            if not isinstance(metadata,dict) or not isinstance(metadata.get('sources'),list) or not all(isinstance(x,str) for x in metadata['sources']) or not isinstance(metadata.get('source_hashes'),dict) or not all(isinstance(k,str) and isinstance(v,str) for k,v in metadata['source_hashes'].items()) or not isinstance(metadata.get('functions'),list):return finish(13)
        except (OSError,ValueError,TypeError):return finish(13)
        report['compiler_profile']='native-c-checked-library';report['compiler_source_map_sha256']=hashlib.sha256(map_bytes).hexdigest()
        names=metadata['sources'];raw=[];limit=self.lib.SC_source_limit()
        if not self.lib.SC_units(len(names)):return finish(2)
        try:
            for name in names:
                with Path(name).open('rb') as stream:raw.append(stream.read(limit+1))
        except OSError:return finish(13)
        lengths=(C.c_int64*len(raw))(*[len(x) for x in raw])
        if not self.lib.SC_inputs(IntView(lengths,len(raw)),0):return finish(2)
        if not self.lib.SC_closure(paths(names),paths(list(metadata['source_hashes']))):return finish(13)
        hashes=metadata['source_hashes'];report['source_sha256']=hashes
        for name,data in zip(names,raw):
            digest=self.source_hash(data)
            if not self.lib.SC_identity(string(hashes.get(name,'')),ByteView(digest,32)):return finish(13,drift_file=name)
        compiled_runs=False
        if run_steps is not None:
            deps=subprocess.run([str(self.compiler),'deps',str(path),'--run-steps',str(run_steps)],capture_output=True,text=True)
            if deps.returncode:return finish(1)
            try:inputs=json.loads(deps.stdout)
            except (ValueError,TypeError):return finish(13)
            if not isinstance(inputs,dict):return finish(13)
            if type(inputs.get('schema_version')) is not int or inputs.get('schema_version')!=1 or type(inputs.get('run_steps')) is not int or inputs.get('run_steps')!=run_steps or inputs.get('descriptors')!=[]:return finish(5)
            if not isinstance(inputs.get('sources'),list) or not all(isinstance(x,str) for x in inputs['sources']) or not self.lib.SC_closure(paths(names),paths(inputs['sources'])):return finish(13)
            report['compiler_dependencies_sha256']=hashlib.sha256(deps.stdout.encode()).hexdigest();report['compiler_descriptors']=[]
            report['run_expression_provenance']='compiled-source-hash-and-dependencies-verified';compiled_runs=True
        unit={name:i for i,name in enumerate(names)};rows=[]
        inventory={'schema':'coax.compiler-resolved-types.v1','functions':[
            {'unit':unit.get(p['file'],-1),'name':p['name'],'line':p['line'],'variables':[
                {key:v[key] for key in ['name','line','role','type']} for v in p['variables']]}
            for p in metadata['functions']]}
        canonical=json.dumps(inventory,sort_keys=True,separators=(',',':')).encode()
        report['resolved_type_sha256']=bytes(self.source_hash(canonical)).hex()
        report['resolved_type_inventory']=inventory
        report['resolved_type_inventory_scope']='compiler-typed physical closure; only reached procedures flow-analyzed'
        for procedure in metadata['functions']:
            for variable in procedure['variables']:
                rows.append(Type(unit.get(procedure['file'],-1),string(procedure['name']),procedure['line'],string(variable['name']),variable['line'],string(variable['role']),string(variable['type'])))
        types=(Type*len(rows))(*rows);sources=(String*len(raw))(*[string(data) for data in raw])
        result=self._analyze(sources,types,string(entry),compiled_runs=compiled_runs)
        # Detect changes after compilation/analysis; this is a bound snapshot,
        # never a promise that the filesystem remains frozen afterward.
        try:
            for name in names:
                with Path(name).open('rb') as stream:data=stream.read(limit+1)
                if len(data)>limit:return finish(13,drift_file=name)
                digest=self.source_hash(data)
                if not self.lib.SC_identity(string(hashes[name]),ByteView(digest,32)):return finish(13,drift_file=name)
        except OSError:return finish(13)
        if metadata_sha(self.compiler)!=self.compiler_fingerprint:return finish(13)
        return finish(result.error,file=names[result.unit] if 0<=result.unit<len(names) else None,line=result.line,column=result.column,reached_procedures=result.reached,visited_nodes=result.visits,return_dependency_mask=result.return_mask)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('source',type=Path);parser.add_argument('--entry',required=True);parser.add_argument('--report',type=Path);parser.add_argument('--run-steps',type=bounded_run_steps)
    args=parser.parse_args();tool=Audit()
    try:
        report=tool.audit(args.source,args.entry,run_steps=args.run_steps);text=json.dumps(report,indent=2)+'\n'
        if args.report:args.report.write_text(text)
        else:print(text,end='')
        raise SystemExit(0 if report['accepted'] else 1)
    finally:tool.close()
