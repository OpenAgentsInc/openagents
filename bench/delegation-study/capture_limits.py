"""Bound reads of executor-controlled files after its process has stopped."""
import time
import math


class CaptureLimit(ValueError):
    pass


class Limits:
    DEFAULTS={'max_entries':100000,'max_file_bytes':128*1024*1024,'max_total_bytes':2*1024*1024*1024,'timeout_s':120}

    def __init__(self,config=None):
        self.config=dict(self.DEFAULTS,**(config or {}))
        if set(self.config)!=set(self.DEFAULTS) or any(type(v) not in (int,float) or not math.isfinite(v) or v<=0 or v>self.DEFAULTS[k] for k,v in self.config.items()):
            raise ValueError('Invalid capture bounds')
        self.deadline=time.monotonic()+self.config['timeout_s']
        self.entries=0;self.bytes=0

    def check(self):
        if time.monotonic()>self.deadline:raise CaptureLimit('capture_deadline')

    def entry(self,size=0):
        self.check();self.entries+=1
        if self.entries>self.config['max_entries']:raise CaptureLimit('capture_entry_count')
        if size>self.config['max_file_bytes']:raise CaptureLimit('capture_file_size')

    def consume(self,count,file_bytes):
        self.check();self.bytes+=count
        if file_bytes>self.config['max_file_bytes']:raise CaptureLimit('capture_file_growth')
        if self.bytes>self.config['max_total_bytes']:raise CaptureLimit('capture_total_bytes')


class Reader:
    def __init__(self,handle,limits):self.handle=handle;self.limits=limits;self.bytes=0
    def read(self,size=-1):
        self.limits.check()
        if size<0 or size>1024*1024:raise CaptureLimit('capture_read_request')
        block=self.handle.read(size)
        self.bytes+=len(block);self.limits.consume(len(block),self.bytes)
        return block


class Writer:
    def __init__(self,handle,deadline,max_bytes):
        self.handle=handle;self.deadline=deadline;self.max_bytes=max_bytes;self.bytes=0
    def write(self,data):
        if time.monotonic()>self.deadline:raise CaptureLimit('capture_deadline')
        if self.bytes+len(data)>self.max_bytes:raise CaptureLimit('capture_payload_bytes')
        count=self.handle.write(data);self.bytes+=count
        return count
    def tell(self):return self.handle.tell()
    def flush(self):return self.handle.flush()
