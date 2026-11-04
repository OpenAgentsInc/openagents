#!/usr/bin/env python3
"""Compares two Granite dumps (granite_run dump, granite_llama): token ids,
each layer's residual stream, the prompt's full-vocabulary logits, and the
greedy continuation with each step's logits. Standard library only.
  granite_parity.py PSIONIC.bin LLAMA.bin [--require] [--json OUT]"""
import array,json,math,struct,sys
def load(path):
    b=open(path,'rb').read();n,layers,width,vocab,steps=struct.unpack_from('<5q',b);at=40
    ids=list(struct.unpack_from(f'<{n}q',b,at));at+=8*n
    res=array.array('f');res.frombytes(b[at:at+4*layers*n*width]);at+=4*layers*n*width
    logits=[];tokens=[]
    l=array.array('f');l.frombytes(b[at:at+4*vocab]);at+=4*vocab;logits.append(l)
    for _ in range(steps):
        tokens.append(struct.unpack_from('<q',b,at)[0]);at+=8
        l=array.array('f');l.frombytes(b[at:at+4*vocab]);at+=4*vocab;logits.append(l)
    return dict(n=n,layers=layers,width=width,vocab=vocab,ids=ids,res=res,logits=logits,tokens=tokens)
def dist(a,b):
    worst=0.0;scale=0.0
    for x,y in zip(a,b):
        d=abs(x-y)
        if d>worst:worst=d
        if abs(y)>scale:scale=abs(y)
    return worst/(1+scale)
def rel_rms(a,b):
    num=sum((x-y)**2 for x,y in zip(a,b));den=sum(y*y for y in b)
    return math.sqrt(num/den) if den else 0.0
def top(l,k=5):return sorted(range(len(l)),key=lambda i:-l[i])[:k]
def softmax(l):
    m=max(l);e=[math.exp(x-m) for x in l];s=sum(e);return [x/s for x in e]
def kl(p,q):return sum(a*math.log(a/b) for a,b in zip(p,q) if a>0 and b>0)
def main():
    p=load(sys.argv[1]);q=load(sys.argv[2]);out={}
    assert p['ids']==q['ids'],'the token ids differ'
    n,w=p['n'],p['width'];layers=[]
    for l in range(p['layers']):
        rows=range(n) if l<p['layers']-1 else [n-1]
        a=[p['res'][(l*n+r)*w+i] for r in rows for i in range(w)];b=[q['res'][(l*n+r)*w+i] for r in rows for i in range(w)]
        layers.append(dict(layer=l,max_rel=dist(a,b),rel_rms=rel_rms(a,b)))
    steps=[]
    for s,(a,b) in enumerate(zip(p['logits'],q['logits'])):
        pa,pb=softmax(a),softmax(b)
        steps.append(dict(step=s,max_rel=dist(a,b),rel_rms=rel_rms(a,b),kl=kl(pb,pa),top_psionic=top(a),top_llama=top(b),argmax_same=top(a,1)==top(b,1)))
    same=0
    for x,y in zip(p['tokens'],q['tokens']):
        if x!=y:break
        same+=1
    out=dict(tokens=n,ids=p['ids'],layers=layers,logits=steps,greedy_psionic=p['tokens'],greedy_llama=q['tokens'],greedy_agree=same,greedy_steps=len(q['tokens']))
    worst=max(layers,key=lambda x:x['rel_rms'])
    print(f"ids: {n} equal; residual stream: worst layer {worst['layer']} relative RMS {worst['rel_rms']:.3g} (max {worst['max_rel']:.3g}); last layer relative RMS {layers[-1]['rel_rms']:.3g}")
    print(f"prompt logits: relative RMS {steps[0]['rel_rms']:.3g}, KL(llama||psionic) {steps[0]['kl']:.3g}, top-5 {steps[0]['top_psionic']} vs {steps[0]['top_llama']}")
    print(f"greedy: {same} of {len(q['tokens'])} tokens agree; worst step logits relative RMS {max(x['rel_rms'] for x in steps):.3g}, worst KL {max(x['kl'] for x in steps):.3g}")
    if '--require' in sys.argv:
        # Bounds for the token-at-a-time comparison (both sides f32 activations).
        bad=[]
        if same!=len(q['tokens']):bad.append('the greedy continuations differ')
        if worst['rel_rms']>5e-3:bad.append('a layer differs by more than 5e-3 relative RMS')
        if max(x['rel_rms'] for x in steps)>5e-3:bad.append('logits differ by more than 5e-3 relative RMS')
        if bad:sys.exit('; '.join(bad))
        print('parity: ok')
    if '--json' in sys.argv:open(sys.argv[sys.argv.index('--json')+1],'w').write(json.dumps(out,indent=1)+'\n')
if __name__=='__main__':main()
