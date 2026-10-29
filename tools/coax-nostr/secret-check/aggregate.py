#!/usr/bin/env python3
"""Aggregate profile audit I/O; source/dataflow/guard decisions remain Coax."""
import argparse
import ctypes as C
import json
from pathlib import Path
from check import Audit,COMPILER,HERE,String,Type,Result,metadata_sha
class Guard(C.Structure):_fields_=[('node',C.c_int64),('unit',C.c_int64),('line',C.c_int64),('column',C.c_int64),('mask',C.c_uint64),('kind',C.c_uint32)]
class AggregateAudit(Audit):
    def __init__(self,compiler=COMPILER,directory=None):
        self.guards=[];super().__init__(compiler,directory,HERE/'aggregate.coax',HERE/'bridge-aggregate.cpp')
        self.inputs[str(Path(__file__).resolve())]=metadata_sha(__file__)
    def _bind_bridge(self):
        self.lib.ag_bridge.argtypes=[C.POINTER(String),C.c_int64,C.POINTER(Type),C.c_int64,String,C.POINTER(Guard),C.c_int64,C.POINTER(C.c_int64)];self.lib.ag_bridge.restype=Result
    def _analyze(self,sources,types,entry):
        guards=(Guard*256)();count=C.c_int64();result=self.lib.ag_bridge(sources,len(sources),types,len(types),entry,guards,256,C.byref(count))
        self.guards=[{'node':x.node,'unit':x.unit,'line':x.line,'column':x.column,'origin_mask':x.mask,'kind':'authored-assertion'} for x in guards[:count.value]];return result
    def audit(self,*args,**kwargs):
        self.guards=[];return super().audit(*args,**kwargs)
    def _decorate_report(self,report):
        report['profile']='aggregate-source-v1';report['aggregate_memory_loops_supported']=True
        report['supported_algorithm_dataflow']=report['error'] in (0,14)
        report['source_status']='analyzed-with-guard-obligations' if report['error']==14 else 'admitted-source' if report['error']==0 else 'refused'
        if report['error']==14:report['error_name']='guard-obligations'
        report['production_crypto_approved']=False
        sources=list(report.get('source_sha256',{}));obligations=[]
        for item in self.guards:
            row=dict(item);unit=row.pop('unit');row['file']=sources[unit] if 0<=unit<len(sources) else None
            row['source_sha256']=report.get('source_sha256',{}).get(row['file']);row['classification']='secret-derived' if row['origin_mask'] else 'public-unproved';obligations.append(row)
        report['authored_guard_obligations']=obligations;return report
if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('source',type=Path);parser.add_argument('--entry',required=True);parser.add_argument('--report',type=Path)
    args=parser.parse_args();tool=AggregateAudit()
    try:
        report=tool.audit(args.source,args.entry);text=json.dumps(report,indent=2)+'\n'
        if args.report:args.report.write_text(text)
        else:print(text,end='')
        raise SystemExit(0 if report['accepted'] else 1)
    finally:tool.close()
