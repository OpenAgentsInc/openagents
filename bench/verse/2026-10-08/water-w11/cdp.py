import socket,struct,json,base64,os,urllib.request,time
class Cdp:
 def __init__(self,url):
  from urllib.parse import urlsplit
  u=urlsplit(url);self.s=socket.create_connection((u.hostname,u.port),10);self.s.settimeout(20);k=base64.b64encode(os.urandom(16)).decode();self.s.sendall(f"GET {u.path} HTTP/1.1\r\nHost: {u.netloc}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {k}\r\nSec-WebSocket-Version: 13\r\n\r\n".encode());h=b""
  while not h.endswith(b"\r\n\r\n"):h+=self.s.recv(1)
  assert b" 101 " in h,h;self.seq=0;self.events=[]
 def exact(self,n):
  b=b""
  while len(b)<n:
   x=self.s.recv(n-len(b))
   if not x:raise EOFError()
   b+=x
  return b
 def send(self,obj):
  b=json.dumps(obj).encode();m=os.urandom(4);n=len(b);head=bytes([129,128|(n if n<126 else 126 if n<65536 else 127)])
  if n>=126:head+=struct.pack("!H" if n<65536 else "!Q",n)
  self.s.sendall(head+m+bytes(v^m[i%4] for i,v in enumerate(b)))
 def recv(self):
  a,b=self.exact(2);n=b&127
  if n==126:n=struct.unpack("!H",self.exact(2))[0]
  if n==127:n=struct.unpack("!Q",self.exact(8))[0]
  assert n<32*1024*1024
  m=self.exact(4) if b&128 else None;d=self.exact(n)
  if m:d=bytes(v^m[i%4] for i,v in enumerate(d))
  if a&15==8:raise EOFError()
  return json.loads(d)
 def call(self,method,params=None):
  self.seq+=1;ident=self.seq;self.send({"id":ident,"method":method,"params":params or {}})
  while True:
   r=self.recv()
   if r.get("id")==ident:
    if "error" in r:raise RuntimeError(r)
    return r.get("result",{})
   self.events.append(r)
 def eval(self,expr):return self.call("Runtime.evaluate",{"expression":expr,"returnByValue":True,"awaitPromise":True})
