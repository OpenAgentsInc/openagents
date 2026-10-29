#!/usr/bin/env python3
"""Aggregate profile audit I/O; source/dataflow/guard decisions remain Coax."""
import argparse
import ctypes as C
import json
import sys
from pathlib import Path
from check import Audit,COMPILER,HERE,String,Type,Result,metadata_sha,bounded_run_steps
UNCLASSIFIED=object()
class Guard(C.Structure):_fields_=[('node',C.c_int64),('unit',C.c_int64),('line',C.c_int64),('column',C.c_int64),('mask',C.c_uint64),('kind',C.c_uint32)]
class AggregateAudit(Audit):
    def __init__(self,compiler=COMPILER,directory=None):
        self.guards=[];super().__init__(compiler,directory,HERE/'aggregate.coax',HERE/'bridge-aggregate.cpp')
        self.inputs[str(Path(__file__).resolve())]=metadata_sha(__file__)
    def _bind_bridge(self):
        common=[C.POINTER(String),C.c_int64,C.POINTER(Type),C.c_int64,String,C.POINTER(Guard),C.c_int64,C.POINTER(C.c_int64)]
        for bridge in [self.lib.ag_bridge,self.lib.ag_bridge_compiled]:bridge.argtypes=common;bridge.restype=Result
        self.lib.ag_bridge_classified.argtypes=common+[C.c_uint64,C.POINTER(String),C.POINTER(String),C.c_int64,C.c_bool];self.lib.ag_bridge_classified.restype=Result
        self.lib.ag_bridge_public_views.argtypes=common+[C.c_uint64,C.POINTER(String),C.POINTER(String),C.c_int64,C.c_bool,C.POINTER(C.c_int64),C.POINTER(C.c_uint32)];self.lib.ag_bridge_public_views.restype=Result
    def _classification_requested(self):return getattr(self,'classification',UNCLASSIFIED) is not UNCLASSIFIED
    def _classification(self,metadata,hashes,entry):
        manifest=getattr(self,'classification',UNCLASSIFIED)
        if manifest is UNCLASSIFIED:return None
        if not isinstance(manifest,dict):raise ValueError('classification identity')
        views=manifest.get('schema')=='coax.secret-classification-views.v2'
        required={'schema','entry','sources','parameters'} | ({'views_disjoint','live_aligned_exclusive','public_only','compiler_sha256','audit_inputs','audit_artifact_sha256','profile'} if views else set())
        if set(manifest)!=required or (not views and manifest.get('schema')!='coax.secret-classification.v1') or manifest['entry']!=entry or manifest['sources']!=hashes:raise ValueError('classification identity')
        if views and (manifest['views_disjoint'] is not True or manifest['live_aligned_exclusive'] is not True or manifest['public_only'] is not True or manifest['compiler_sha256']!=self.compiler_fingerprint or manifest['audit_inputs']!=self.inputs or manifest['audit_artifact_sha256']!=self.artifact_fingerprint or manifest['profile']!='native-c-checked-library'):raise ValueError('view premises or tool identity')
        functions=[f for f in metadata['functions'] if f['name']==entry]
        if len(functions)!=1 or functions[0]['file'] not in hashes:raise ValueError('entry identity')
        parameters=[v for v in functions[0]['variables'] if v['role']=='parameter']
        rows=manifest['parameters']
        if not isinstance(rows,list) or len(rows)!=len(parameters) or not 1<=len(rows)<=8:raise ValueError('parameter count')
        mask=0
        for i,(row,actual) in enumerate(zip(rows,parameters)):
            if not isinstance(row,dict) or set(row)!={'name','type','classification'} | ({'view'} if views else set()) or row['name']!=actual['name'] or row['type']!=actual['type'] or row['classification'] not in ('public','secret'):raise ValueError('parameter identity')
            if views:
                view=row['view'];is_view=actual['type'].startswith('[] ')
                if is_view:
                    if not isinstance(view,dict) or set(view)!={'element_type','extent','access'} or view['element_type']!=actual['type'][3:] or type(view['extent']) is not int or not 1<=view['extent']<=1024 or view['access'] not in ('read','write'):raise ValueError('view identity')
                elif view is not None:raise ValueError('non-view premise')
            if row['classification']=='secret':mask|=1<<i
        canonical=json.dumps(manifest,sort_keys=True,separators=(',',':')).encode()
        import hashlib
        return {'parameters':rows,'mask':mask,'sha256':hashlib.sha256(canonical).hexdigest(),'views':views}
    def _analyze(self,sources,types,entry,compiled_runs=False,classification=None,string=None):
        guards=(Guard*256)();count=C.c_int64()
        if classification is None:
            bridge=self.lib.ag_bridge_compiled if compiled_runs else self.lib.ag_bridge;result=bridge(sources,len(sources),types,len(types),entry,guards,256,C.byref(count))
        else:
            rows=classification['parameters'];names=(String*len(rows))(*[string(x['name']) for x in rows]);parameter_types=(String*len(rows))(*[string(x['type']) for x in rows])
            if classification['views']:
                extents=(C.c_int64*len(rows))(*[(row['view']['extent'] if row['view'] else 0) for row in rows]);access=(C.c_uint32*len(rows))(*[({'read':1,'write':2}[row['view']['access']] if row['view'] else 0) for row in rows])
                result=self.lib.ag_bridge_public_views(sources,len(sources),types,len(types),entry,guards,256,C.byref(count),classification['mask'],names,parameter_types,len(rows),compiled_runs,extents,access)
            else:result=self.lib.ag_bridge_classified(sources,len(sources),types,len(types),entry,guards,256,C.byref(count),classification['mask'],names,parameter_types,len(rows),compiled_runs)
        self.guards=[{'node':x.node,'unit':x.unit,'line':x.line,'column':x.column,'origin_mask':x.mask,'kind':'authored-assertion'} for x in guards[:count.value]];return result
    def audit(self,*args,classification=UNCLASSIFIED,**kwargs):
        self.guards=[]
        if classification is UNCLASSIFIED:self.classification=UNCLASSIFIED
        else:
            try:self.classification=json.loads(json.dumps(classification,allow_nan=False))
            except (TypeError,ValueError):self.classification=None
        try:return super().audit(*args,**kwargs)
        finally:self.classification=UNCLASSIFIED
    def _decorate_report(self,report):
        report['profile']='aggregate-source-v1';report['aggregate_memory_loops_supported']=True
        report['supported_algorithm_dataflow']=report['error'] in (0,14)
        report['source_status']='analyzed-with-guard-obligations' if report['error']==14 else 'admitted-source' if report['error']==0 else 'refused'
        if report['error']==14:report['error_name']='guard-obligations'
        report['production_crypto_approved']=False
        if 'classified_parameters' in report and isinstance(self.classification,dict) and self.classification.get('schema')=='coax.secret-classification-views.v2':
            report['view_assumption']='caller supplies live aligned fixed-extent views, mutually disjoint from each other and all other reachable storage, with declared access and initial classifications and no concurrent mutation throughout the invocation'
            report['public_only_scope']='all reached public-only bodies and transitive effects are analyzed; no declassification from names'
        sources=list(report.get('source_sha256',{}));obligations=[]
        for item in self.guards:
            row=dict(item);unit=row.pop('unit');row['file']=sources[unit] if 0<=unit<len(sources) else None
            row['source_sha256']=report.get('source_sha256',{}).get(row['file']);row['classification']='secret-derived' if row['origin_mask'] else 'public-unproved';obligations.append(row)
        report['authored_guard_obligations']=obligations;return report
def unique_object(pairs):
    result={}
    for key,value in pairs:
        if key in result:raise ValueError('duplicate classification key: '+key)
        result[key]=value
    return result

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('source',type=Path);parser.add_argument('--entry',required=True);parser.add_argument('--report',type=Path);parser.add_argument('--run-steps',type=bounded_run_steps)
    classification=parser.add_mutually_exclusive_group();classification.add_argument('--classification',type=Path);classification.add_argument('--classification-stdio',action='store_true')
    args=parser.parse_args();tool=AggregateAudit()
    try:
        kwargs={'run_steps':args.run_steps}
        if args.classification:kwargs['classification']=json.loads(args.classification.read_text(),object_pairs_hook=unique_object)
        if args.classification_stdio:
            # The caller explicitly constructs the manifest after seeing this
            # exact in-process artifact identity; no classifications are inferred.
            baseline=tool.audit(args.source,args.entry,**kwargs)
            if 'source_sha256' not in baseline or 'compiler_profile' not in baseline:raise ValueError('baseline source identity unavailable')
            hello={'schema':'coax.secret-classification-handshake.v1','entry':args.entry,'sources':baseline['source_sha256'],'compiler_sha256':baseline['compiler_sha256'],'audit_inputs':baseline['audit_input_sha256'],'audit_artifact_sha256':baseline['audit_artifact_sha256'],'profile':baseline['compiler_profile'],'baseline_error':baseline['error_name']}
            print(json.dumps(hello,separators=(',',':')),flush=True)
            line=sys.stdin.readline(1048577)
            if not line or len(line)>1048576 or not line.endswith('\n'):raise ValueError('classification line unavailable or too large')
            kwargs['classification']=json.loads(line,object_pairs_hook=unique_object)
        report=tool.audit(args.source,args.entry,**kwargs);text=json.dumps(report,indent=2)+'\n'
        if args.report:args.report.write_text(text)
        else:print(text,end='')
        raise SystemExit(0 if report['accepted'] else 1)
    finally:tool.close()
