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

class Audit:
    def __init__(self,compiler=COMPILER,directory=None):
        self.compiler=Path(compiler).resolve();self.temporary=tempfile.TemporaryDirectory(prefix='coax-secret-check-') if directory is None else None
        self.directory=Path(self.temporary.name) if self.temporary else Path(directory);self.directory.mkdir(parents=True,exist_ok=True)
        for name,source in [('frontend',HERE/'frontend.coax'),('policy',ROOT/'coax/lib/crypto/secret-flow.coax')]:
            output=self.directory/(name+'.c');output.write_text(run(self.compiler,'c',source,'--library','--header',self.directory/(name+'.h')).stdout)
            run('cc','-std=c11','-O2','-fPIC','-c',output,'-o',self.directory/(name+'.o'))
        self.artifact=self.directory/'secret-check.dylib'
        run('c++','-std=c++20','-O2','-shared','-fPIC','-I'+str(self.directory),self.directory/'frontend.o',self.directory/'policy.o',HERE/'bridge.cpp','-o',self.artifact)
        self.lib=C.CDLL(str(self.artifact))
        self.lib.SC_source_limit.restype=C.c_int64
        self.lib.SC_units.argtypes=[C.c_int64];self.lib.SC_units.restype=C.c_bool
        self.lib.SC_inputs.argtypes=[IntView,C.c_int64];self.lib.SC_inputs.restype=C.c_bool
        self.lib.SC_identity.argtypes=[String,ByteView];self.lib.SC_identity.restype=C.c_bool
        self.lib.SC_closure.argtypes=[PathView,PathView];self.lib.SC_closure.restype=C.c_bool
        self.lib.SC_error_name.argtypes=[C.c_uint32];self.lib.SC_error_name.restype=String
        self.lib.sha256_hash.argtypes=[ByteView,ByteView];self.lib.sha256_hash.restype=C.c_uint32
        self.lib.sc_bridge.argtypes=[C.POINTER(String),C.c_int64,C.POINTER(Type),C.c_int64,String];self.lib.sc_bridge.restype=Result
        # Provenance metadata only; source identity admission uses Coax SHA/match.
        self.inputs={}
        for source in [HERE/'frontend.coax',ROOT/'coax/lib/crypto/secret-flow.coax']:
            for path in json.loads(run(self.compiler,'deps',source).stdout)['sources']:self.inputs[path]=metadata_sha(path)
        for path in [HERE/'bridge.cpp',Path(__file__)]:self.inputs[str(path)]=metadata_sha(path)
    def close(self):
        if self.temporary:self.temporary.cleanup()
    def source_hash(self,data):
        source=(C.c_uint8*len(data)).from_buffer_copy(data);digest=(C.c_uint8*32)()
        if self.lib.sha256_hash(ByteView(source,len(data)),ByteView(digest,32))!=0:raise ValueError('Coax source hashing refused')
        return digest
    def audit(self,path,entry,after_compile=None):
        path=Path(path).resolve();keep=[]
        def string(value):
            data=value if isinstance(value,bytes) else value.encode();buffer=C.create_string_buffer(data);keep.append(buffer)
            return String(C.cast(buffer,C.c_void_p),len(data))
        def paths(values):
            array=(PathValue*len(values))(*[PathValue(string(x)) for x in values]);keep.append(array);return PathView(array,len(values))
        compiler_hash=metadata_sha(self.compiler)
        report={'schema':'coax.secret-source-audit.v1','entry':entry,'classification':'all entry parameters secret','profile':'unsigned-scalar-source-v1','accepted':False,
                'compiler_sha256':compiler_hash,'compiler':run(self.compiler,'--version').stdout.strip(),'audit_artifact_sha256':metadata_sha(self.artifact),
                'audit_input_sha256':self.inputs,'object_timing_approved':False,'checked_runtime_guards_proved':False,'aggregate_memory_loops_supported':False}
        def finish(error,**extra):
            name=self.lib.SC_error_name(error);report.update(error=error,error_name=C.string_at(name.data,name.count).decode(),accepted=error==0,**extra);return report
        source_map=self.directory/'source-map.json'
        checked=subprocess.run([str(self.compiler),'c',str(path),'--library','--source-map',str(source_map)],capture_output=True,text=True)
        if checked.returncode:return finish(1)
        if after_compile:after_compile()
        # The real typed compiler's source map is its resolved source inventory.
        # Standalone `deps` also requires an executable main on private libraries.
        # Descriptor import syntax is refused by the unmodified parser below.
        metadata=json.loads(source_map.read_text());names=metadata['sources'];raw=[];limit=self.lib.SC_source_limit()
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
        unit={name:i for i,name in enumerate(names)};rows=[]
        for procedure in metadata['functions']:
            for variable in procedure['variables']:
                rows.append(Type(unit.get(procedure['file'],-1),string(procedure['name']),procedure['line'],string(variable['name']),variable['line'],string(variable['role']),string(variable['type'])))
        types=(Type*len(rows))(*rows);sources=(String*len(raw))(*[string(data) for data in raw])
        result=self.lib.sc_bridge(sources,len(raw),types,len(rows),string(entry))
        # Detect changes after compilation/analysis; this is a bound snapshot,
        # never a promise that the filesystem remains frozen afterward.
        try:
            for name in names:
                with Path(name).open('rb') as stream:data=stream.read(limit+1)
                if len(data)>limit:return finish(13,drift_file=name)
                digest=self.source_hash(data)
                if not self.lib.SC_identity(string(hashes[name]),ByteView(digest,32)):return finish(13,drift_file=name)
        except OSError:return finish(13)
        return finish(result.error,file=names[result.unit] if 0<=result.unit<len(names) else None,line=result.line,column=result.column,reached_procedures=result.reached,visited_nodes=result.visits,return_dependency_mask=result.return_mask)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('source',type=Path);parser.add_argument('--entry',required=True);parser.add_argument('--report',type=Path)
    args=parser.parse_args();tool=Audit()
    try:
        report=tool.audit(args.source,args.entry);text=json.dumps(report,indent=2)+'\n'
        if args.report:args.report.write_text(text)
        else:print(text,end='')
        raise SystemExit(0 if report['accepted'] else 1)
    finally:tool.close()
