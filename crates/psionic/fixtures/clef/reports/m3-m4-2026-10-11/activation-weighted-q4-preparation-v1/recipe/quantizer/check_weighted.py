#!/usr/bin/env python3
"""Synthetic weighted-Q4 byte/decoder checks; never reads model tensor payloads."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import time
import numpy as np
from weighted_q4 import BASE,WeightedQ4,require,sha
sys.path.insert(0,str(BASE/'cpu27b-diagnostic/streamed-gguf'))
from gguf_import import decoder_classes


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);a=p.parse_args();require(not a.output.exists(),'preserve earlier evidence')
    quantizer=WeightedQ4();classes,decoder_identity=decoder_classes();decode=classes['Q4_K'].dequantize_blocks
    report={'status':'running','scope':'Synthetic packing and decoder checks; no model execution or accuracy claim.','model_payload_bytes_read':0,'gpu_executed':False,'quantizer':quantizer.identity,'independent_decoder':decoder_identity,'cases':[],'refusals':[]};began=time.monotonic()
    try:
        for width in (5120,17408):
            for mode in ('zero','ones','sparse_blocks','mixed'):
                rows=3;indices=np.arange(rows*width,dtype=np.uint32).reshape(rows,width)
                source=((indices%257).astype(np.float32)-128)/128
                source*=np.array([1/64,1,8],dtype=np.float32)[(indices//256)%3]
                importance=np.ones(width,dtype=np.float32)
                if mode=='zero':importance.fill(0)
                elif mode=='sparse_blocks':importance[np.arange(width)//256%2==0]=0
                elif mode=='mixed':importance=np.array([0,1/1024,1,16],dtype=np.float32)[np.arange(width)%4]
                packed=quantizer.quantize(source,importance)
                separate=np.concatenate([quantizer.quantize(source[i:i+1],importance) for i in range(rows)])
                require(np.array_equal(packed,separate),'row grouping changed weighted Q4 bytes')
                native=quantizer.decode_packed(packed,width)
                independent=decode(packed.reshape(-1,144)).reshape(-1).astype(np.float32)
                require(np.isfinite(independent).all() and np.array_equal(native.view(np.uint32),independent.view(np.uint32)),'independent and C Q4 decoding differs')
                if mode=='zero':require(np.all(native==0),'all-zero importance did not yield finite zero decoded blocks')
                if mode=='sparse_blocks':require(np.all(native.reshape(rows,-1,256)[:,::2,:]==0),'zero-importance blocks did not decode to zero')
                error=(native.reshape(rows,width).astype(np.float64)-source)**2
                report['cases'].append({'width':width,'rows':rows,'importance':mode,'values':rows*width,'packed_bytes':len(packed),'packed_sha256':sha(packed.tobytes()),'decoded_sha256':sha(native.tobytes()),'rowwise_packed_exact':True,'independent_decode_exact':True,'guard_bytes_unchanged':True,'finite':True,'weighted_squared_error':float((error*importance).sum())})
        source=np.ones((1,5120),dtype=np.float32)
        for name,value in [('negative',-1),('nan',np.nan),('infinity',np.inf)]:
            importance=np.ones(5120,dtype=np.float32);importance[0]=value
            try:quantizer.quantize(source,importance)
            except ValueError:report['refusals'].append(name)
            else:raise ValueError('invalid importance was not refused')
        report.update(status='passed',compared_values=sum(x['values'] for x in report['cases']))
    except BaseException as error:report.update(status='failed',error_type=type(error).__name__,error=str(error));raise
    finally:
        report['seconds']=time.monotonic()-began;a.output.write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
    print(json.dumps({'status':report['status'],'compared_values':report['compared_values'],'seconds':report['seconds']}))


if __name__=='__main__':main()
