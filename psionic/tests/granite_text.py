#!/usr/bin/env python3
"""Checks Psionic's Granite tokenizer and chat renderer against the recorded
Hugging Face vectors (standard library only), and the GGUF's vocabulary and
merges against the pinned tokenizer.json.
  granite_text.py --cases OUT.bin          write the cases for granite_text.c
  granite_text.py --check RESULTS [MODEL_DIR GGUF]"""
import json,struct,sys
from pathlib import Path
FIX=Path(__file__).resolve().parents[1]/'fixtures/granite'
def load():return json.loads((FIX/'tokenizer-vectors.json').read_text()),json.loads((FIX/'chat-vectors.json').read_text())
def pack(s):b=s.encode();return struct.pack('<I',len(b))+b
def cases(path):
    texts,chats=load();out=b''
    for v in texts:out+=struct.pack('<I',0)+pack(v['text'])
    for c in chats:
        o=c['options'];out+=struct.pack('<I',1)+bytes([c['generation'],o.get('enable_thinking',True),o.get('low_effort',False)])+struct.pack('<I',len(c['messages']))
        for m in c['messages']:out+=pack(m['role'])+pack(m['content'])
    Path(path).write_bytes(out)
def gguf_text(path):
    f=open(path,'rb');rd=f.read
    def u(fmt):return struct.unpack('<'+fmt,rd(struct.calcsize(fmt)))[0]
    def s():return rd(u('Q'))
    T={0:'B',1:'b',2:'H',3:'h',4:'I',5:'i',6:'f',7:'?',10:'Q',11:'q',12:'d'}
    def val(t):
        if t==8:return s()
        if t==9:it=u('I');n=u('Q');return [val(it) for _ in range(n)]
        return u(T[t])
    rd(4);u('I');u('Q');nm=u('Q');M={}
    for _ in range(nm):k=s().decode();M[k]=val(u('I'))
    return M
def check(results,model=None,gguf=None):
    texts,chats=load();lines=Path(results).read_text().splitlines();head=lines[0];lines=lines[1:];bad=0
    assert head.endswith('chapter 52 refuses: whitespace control'),head
    for v,line in zip(texts,lines[:len(texts)]):
        got=[int(x) for x in line.split()[1:]] if 'refused' not in line and 'roundtrip' not in line else None
        if got!=v['ids']:bad+=1;print('tokenizer differs:',repr(v['text']),line[:120])
    for c,line in zip(chats,lines[len(texts):]):
        parts=line.split();text=bytes.fromhex(parts[1]).decode() if len(parts)>1 and parts[1]!='refused' else None
        got=[int(x) for x in parts[2:]] if text is not None and 'roundtrip-failed' not in parts else None
        if text!=c['text'] or got!=c['ids']:bad+=1;print('chat differs:',c['messages'],c['options'],repr(text))
    assert len(lines)==len(texts)+len(chats),(len(lines),len(texts)+len(chats))
    print(f'{len(texts)} texts and {len(chats)} chat renders: {len(texts)+len(chats)-bad} equal Hugging Face token for token')
    if model:
        tj=json.loads((Path(model)/'tokenizer.json').read_text());M=gguf_text(gguf)
        vocab={i:t for t,i in tj['model']['vocab'].items()}
        for a in tj['added_tokens']:vocab[a['id']]=a['content']
        tokens=[t.decode() for t in M['tokenizer.ggml.tokens']];merges=[m.decode() for m in M['tokenizer.ggml.merges']]
        want=[' '.join(m) if isinstance(m,list) else m for m in tj['model']['merges']]
        assert len(tokens)==len(vocab)==100352 and all(tokens[i]==vocab[i] for i in range(len(tokens))),'vocabulary differs from tokenizer.json'
        assert merges==want,'merges differ from tokenizer.json'
        added={a['id'] for a in tj['added_tokens']};types=M['tokenizer.ggml.token_type']
        assert {i for i,t in enumerate(types) if t in (3,4)}==added,'added tokens differ'
        print(f'the GGUF vocabulary ({len(tokens)}), merges ({len(merges)}) and added tokens ({len(added)}) equal tokenizer.json')
    if bad:sys.exit(1)
if __name__=='__main__':
    if sys.argv[1]=='--cases':cases(sys.argv[2])
    else:check(*sys.argv[2:])
