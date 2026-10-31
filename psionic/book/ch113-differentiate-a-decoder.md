# 113. Differentiate a decoder

Chapter 112's bigram sees one byte. A decoder sees the whole prefix. This
chapter builds the smallest complete one, runs it two ways (all positions at
once, and one token at a time from a bounded key/value cache), derives every
gradient by hand and checks each against a central difference.

The runnable profile is `cpu-f64-causal-decoder-v1`: binary64 on the CPU, no
pretrained weights, no GPU. Shapes are bounded: at most 256 tokens of
vocabulary, 8 query heads, a width of 128, 256 hidden FFN units, 4 blocks and
64 positions. The library is
[decoder.coax](../lib/transformer/decoder.coax).

## The model

A token's embedding row is the first residual. Each block normalizes, attends,
adds; normalizes, applies a feed-forward network, adds. A final RMSNorm and an
untied vocabulary matrix give the logits.

- **RMSNorm:** `y = g * x / sqrt(mean(x^2) + 1e-6)`.
- **Grouped-query attention:** `heads` query heads share `kv_heads` key/value
  heads; query head `h` reads key/value head `h / (heads / kv_heads)`. Scores
  are `q.k / sqrt(head_width)` over positions `0..t` only, then a softmax
  shifted by its maximum.
- **RoPE:** each interleaved pair `(2i, 2i+1)` of a query or key head turns by
  `t * 10000^(-2i/head_width)` radians.
- **FFN:** `w2 * relu(w1 * x)^2`, the squared ReLU nanochat uses.
- **Loss:** mean next-token cross entropy over positions `0..n-2`; the last
  token predicts nothing.

<!-- source: psionic/lib/transformer/decoder.coax shape -->
```jai
TdShape :: struct {vocab:int;width:int;heads:int;kv_heads:int;head_width:int;hidden:int;layers:int;context:int;}
```

Parameters live in one flat binary64 array: the embedding, each block's
`norm1, wq, wk, wv, wo, norm2, w1, w2`, the final norm, then the head.
Saved values have a fixed place per block and position, so the forward pass
can stop at any position and the backward pass knows where to look.

<!-- source: psionic/lib/transformer/decoder.coax layout -->
```jai
td_valid :: (s:TdShape)->bool {
    if s.vocab<2 || s.vocab>256 || s.heads<1 || s.heads>8 || s.kv_heads<1 || s.kv_heads>s.heads || s.heads%s.kv_heads!=0 {return false;}
    if s.head_width<2 || s.head_width>32 || s.head_width%2!=0 || s.width!=s.heads*s.head_width || s.width>128 {return false;}
    return s.hidden>=1 && s.hidden<=256 && s.layers>=1 && s.layers<=4 && s.context>=1 && s.context<=64;
}
td_block_size :: (s:TdShape)->int {d:=s.width;q:=s.heads*s.head_width;k:=s.kv_heads*s.head_width;return 2*d+q*d+2*k*d+d*q+2*s.hidden*d;}
td_parameter_count :: (s:TdShape)->int {return 2*s.vocab*s.width+s.layers*td_block_size(s)+s.width;}
td_block :: (s:TdShape,layer:int)->TdBlock {
    d:=s.width;q:=s.heads*s.head_width;k:=s.kv_heads*s.head_width;b:TdBlock;b.norm1=s.vocab*d+layer*td_block_size(s);
    b.wq=b.norm1+d;b.wk=b.wq+q*d;b.wv=b.wk+k*d;b.wo=b.wv+k*d;b.norm2=b.wo+d*q;b.w1=b.norm2+d;b.w2=b.w1+s.hidden*d;return b;
}
td_final_norm :: (s:TdShape)->int {return s.vocab*s.width+s.layers*td_block_size(s);}
td_unembed :: (s:TdShape)->int {return td_final_norm(s)+s.width;}
// Saved values for one layer at one position: input, normed input, two
// inverse RMS values, rotated queries, attention context, the mid residual,
// its norm, the FFN pre-activation, then per-head attention probabilities.
td_slot_size :: (s:TdShape)->int {q:=s.heads*s.head_width;return 4*s.width+2+2*q+s.hidden+s.heads*s.context;}
td_layer_size :: (s:TdShape)->int {return s.context*(td_slot_size(s)+2*s.kv_heads*s.head_width);}
td_work_count :: (s:TdShape)->int {return s.layers*td_layer_size(s)+s.context*(2*s.width+1+s.vocab);}
```

## One position at a time

`td_advance` computes every block at position `t`. It reads the token and the
keys and values already saved for positions `0..t-1`, then writes position
`t`'s own. Nothing at a later position is read, so a future token cannot
change a past output. `td_forward` calls it for `t = 0, 1, ...`; `td_decode`
calls it once for the next position of a cache that already holds the past.

<!-- source: psionic/lib/transformer/decoder.coax advance -->
```jai
// Computes every layer and the logits at position t from the token and the
// keys and values already saved for positions 0..t-1. Nothing later is read.
td_advance :: (s:TdShape,p:[]f64,w:[]f64,token:int,t:int) {
    d:=s.width;e:=s.head_width;q:=s.heads*e;k:=s.kv_heads*e;group:=s.heads/s.kv_heads;scale:=1/sqrt(f64(e));
    first:=td_slot(s,0,t);for i:0..<d {w[first+i]=p[token*d+i];}
    for l:0..<s.layers {
        b:=td_block(s,l);o:=td_slot(s,l,t);n1:=o+d;r:=o+2*d;qs:=r+2;cs:=qs+q;m:=cs+q;n2:=m+d;u:=n2+d;a:=u+s.hidden;
        key:=td_keys(s,l)+t*k;value:=td_values(s,l)+t*k;
        w[r]=td_norm(w,o,p,b.norm1,d,w,n1);
        td_matvec(p,b.wq,q,d,w,n1,w,qs);td_matvec(p,b.wk,k,d,w,n1,w,key);td_matvec(p,b.wv,k,d,w,n1,w,value);
        td_rope(w,qs,s.heads,e,f64(t));td_rope(w,key,s.kv_heads,e,f64(t));
        for h:0..<s.heads {g:=h/group;row:=a+h*s.context;top:=0.0;
            for j:0..<t+1 {score:=0.0;for x:0..<e {score+=w[qs+h*e+x]*w[td_keys(s,l)+j*k+g*e+x];}w[row+j]=score*scale;if j==0 || w[row+j]>top {top=w[row+j];}}
            total:=0.0;for j:0..<t+1 {w[row+j]=exp(w[row+j]-top);total+=w[row+j];}for j:0..<t+1 {w[row+j]/=total;}
            for x:0..<e {acc:=0.0;for j:0..<t+1 {acc+=w[row+j]*w[td_values(s,l)+j*k+g*e+x];}w[cs+h*e+x]=acc;}
        }
        td_matvec(p,b.wo,d,q,w,cs,w,m);for i:0..<d {w[m+i]+=w[o+i];}
        w[r+1]=td_norm(w,m,p,b.norm2,d,w,n2);td_matvec(p,b.w1,s.hidden,d,w,n2,w,u);
        next:=td_final(s,t);if l+1<s.layers {next=td_slot(s,l+1,t);}
        for i:0..<d {acc:=0.0;for f:0..<s.hidden {z:=w[u+f];if z>0 {acc+=p[b.w2+i*s.hidden+f]*z*z;}}w[next+i]=w[m+i]+acc;}
    }
    f:=td_final(s,t);w[f+2*d]=td_norm(w,f,p,td_final_norm(s),d,w,f+d);td_matvec(p,td_unembed(s),s.vocab,d,w,f+d,w,f+2*d+1);
}
```

<!-- source: psionic/lib/transformer/decoder.coax decode -->
```jai
// One incremental step: `filled` positions are already in the cache; this
// writes position `filled` and returns its logits.
td_decode :: (s:TdShape,p:[]f64,cache:[]f64,filled:int,token:int,logits:[]f64,overlap:bool)->TdResult #export_c {
    status:=td_admit(s,p,cache);if status!=TdStatus.ok {return TdResult.{status=status};}
    if filled<0 || filled>=s.context {return TdResult.{status=TdStatus.position};}if token<0 || token>=s.vocab {return TdResult.{status=TdStatus.token};}
    if logits.count<s.vocab {return TdResult.{status=TdStatus.capacity};}if overlap {return TdResult.{status=TdStatus.alias};}
    td_advance(s,p,cache,token,filled);z:=td_final(s,filled)+2*s.width+1;for v:0..<s.vocab {logits[v]=cache[z+v];}
    return TdResult.{written=filled+1};
}
```

The checks run both paths on the same weights. Each decoded step's logits are
bit-for-bit those of the full sequence. Changing token `j` leaves every logit
before `j` bit-identical and changes position `j`'s. A cache that is full
refuses another step (`position`).

An independent definition, [decoder_oracle.py](../tests/decoder_oracle.py), is
written from the equations above rather than from the Coax code. It builds
the parameters by name and attends over the whole prefix for each position.
On a deterministic tiny model (5 tokens, width 4, two query heads sharing one
key/value head, two blocks, four positions) the twenty Coax logits match its
values within `1e-12`, and so does the loss.

## The backward pass

`td_gradients` runs the forward pass, then walks back. The head gives
`(softmax - onehot) / (n-1)` per prediction. Each linear map `y = W x` adds
`dy x^T` to its weight gradient and `W^T dy` to its input. RMSNorm with
`r = 1/rms(x)` adds `dy * x * r` to its gains and
`r * g * dy - x * r^3 * sum(g * dy * x) / n` to its input. The squared ReLU
gives `2 * relu(z)`. For attention with probabilities `p`, score gradients are
`p * (dp - sum(p * dp))`, scaled once by `1/sqrt(head_width)`. A position's key
and value collect gradient from every later query that read them, so a block
first finishes attention for every position, then undoes RoPE (the rotation
by `-t`) and the projections. The embedding row of each input token collects
its first block's input gradient.

<!-- source: psionic/lib/transformer/decoder.coax backward -->
```jai
td_gradients :: (s:TdShape,p:[]f64,tokens:[]int,w:[]f64,dw:[]f64,grads:[]f64,overlap:bool)->TdResult #export_c {
    status:=td_admit(s,p,w);if status!=TdStatus.ok {return TdResult.{status=status};}
    status=td_tokens(s,tokens);if status!=TdStatus.ok {return TdResult.{status=status};}if tokens.count<2 {return TdResult.{status=TdStatus.shape};}
    if dw.count<td_work_count(s) || grads.count!=p.count {return TdResult.{status=TdStatus.capacity};}if overlap {return TdResult.{status=TdStatus.alias};}
    n:=tokens.count;d:=s.width;e:=s.head_width;q:=s.heads*e;k:=s.kv_heads*e;group:=s.heads/s.kv_heads;scale:=1/sqrt(f64(e));
    for t:0..<n {td_advance(s,p,w,tokens[t],t);}
    for i:0..<grads.count {grads[i]=0;}for i:0..<td_work_count(s) {dw[i]=0;}
    // Head: softmax minus the one-hot target, averaged over the n-1 predictions.
    for t:0..<n-1 {f:=td_final(s,t);z:=f+2*d+1;top:=w[z];for v:1..<s.vocab {if w[z+v]>top {top=w[z+v];}}sum:=0.0;for v:0..<s.vocab {sum+=exp(w[z+v]-top);}
        for v:0..<s.vocab {dw[z+v]=exp(w[z+v]-top)/sum/f64(n-1);}dw[z+tokens[t+1]]-=1/f64(n-1);
        td_matvec_back(p,td_unembed(s),s.vocab,d,w,f+d,dw,z,grads,dw,f+d);td_norm_back(dw,f+d,w,f,w[f+2*d],p,td_final_norm(s),d,grads,dw,f);}
    l:=s.layers;
    while l>0 {l-=1;b:=td_block(s,l);keys:=td_keys(s,l);values:=td_values(s,l);
        for t:0..<n {o:=td_slot(s,l,t);n1:=o+d;r:=o+2*d;qs:=r+2;cs:=qs+q;m:=cs+q;n2:=m+d;u:=n2+d;a:=u+s.hidden;
            next:=td_final(s,t);if l+1<s.layers {next=td_slot(s,l+1,t);}
            for i:0..<d {dw[m+i]+=dw[next+i];}
            for f:0..<s.hidden {z:=w[u+f];if z>0 {g:=0.0;for i:0..<d {grads[b.w2+i*s.hidden+f]+=dw[next+i]*z*z;g+=p[b.w2+i*s.hidden+f]*dw[next+i];}dw[u+f]=2*z*g;}}
            td_matvec_back(p,b.w1,s.hidden,d,w,n2,dw,u,grads,dw,n2);td_norm_back(dw,n2,w,m,w[r+1],p,b.norm2,d,grads,dw,m);
            td_matvec_back(p,b.wo,d,q,w,cs,dw,m,grads,dw,cs);
            for h:0..<s.heads {g:=h/group;row:=a+h*s.context;mean:=0.0;
                for j:0..<t+1 {dp:=0.0;for x:0..<e {dp+=dw[cs+h*e+x]*w[values+j*k+g*e+x];dw[values+j*k+g*e+x]+=w[row+j]*dw[cs+h*e+x];}dw[row+j]=dp;mean+=w[row+j]*dp;}
                for j:0..<t+1 {ds:=w[row+j]*(dw[row+j]-mean)*scale;for x:0..<e {dw[qs+h*e+x]+=ds*w[keys+j*k+g*e+x];dw[keys+j*k+g*e+x]+=ds*w[qs+h*e+x];}}
            }
        }
        for t:0..<n {o:=td_slot(s,l,t);n1:=o+d;r:=o+2*d;qs:=r+2;m:=qs+2*q;key:=keys+t*k;value:=values+t*k;
            td_rope(dw,qs,s.heads,e,-f64(t));td_rope(dw,key,s.kv_heads,e,-f64(t));
            td_matvec_back(p,b.wq,q,d,w,n1,dw,qs,grads,dw,n1);td_matvec_back(p,b.wk,k,d,w,n1,dw,key,grads,dw,n1);td_matvec_back(p,b.wv,k,d,w,n1,dw,value,grads,dw,n1);
            for i:0..<d {dw[o+i]+=dw[m+i];}td_norm_back(dw,n1,w,o,w[r],p,b.norm1,d,grads,dw,o);
        }
    }
    for t:0..<n {o:=td_slot(s,0,t);for i:0..<d {grads[tokens[t]*d+i]+=dw[o+i];}}
    return TdResult.{written=grads.count,loss=td_loss(s,w,tokens)};
}
```

Every one of the tiny model's 204 parameters is checked against
`(L(p+h) - L(p-h)) / 2h` with `h = 1e-5`, within `1e-8` after scaling by
`1 + |g|`. Every parameter group of both blocks and the head receives gradient.
The embedding rows of token 4, which never appears, and of the final token,
which predicts nothing, are exactly zero.

## A full training run

The small check trains every parameter of the tiny model on one four-token
sequence for 60 plain gradient steps; the loss falls below 5% of its start and
greedy decoding from the first token reproduces the sequence.

[The example](../examples/causal-decoder.coax) trains a larger model (3856 parameters) on an alternating-copy
task: `a b a b a b a b` with `a != b` over six symbols. From position 2 the
next token is the one two places back, so only attention can predict it.
Pairs with `b = a + 1 (mod 6)` are held out. It uses [chapter 124's AdamW](ch124-adamw-and-explicit-precision.md)
over full-batch gradients for 120 steps, then continues each held-out
two-token prompt for six tokens through the KV cache.

<!-- source: psionic/examples/causal-decoder.coax example -->
```jai
// Alternating copy: every sequence is a,b,a,b,a,b,a,b with a != b drawn from
// six symbols. From position 2 on, the next token is the one two places back,
// so only attention over the context can predict it; six pairs are held out.
CD_SHAPE :: TdShape.{vocab=6,width=16,heads=2,kv_heads=1,head_width=8,hidden=32,layers=2,context=8};
cd_held_out :: (a:int,b:int)->bool {return b==(a+1)%6;}
cd_sequence :: (a:int,b:int,out:[]int) {for t:0..<8 {out[t]=a;if t%2==1 {out[t]=b;}}}
// The vocabulary head alone, reading only the current token's embedding.
cd_head_only :: (s:TdShape,p:[]f64,token:int)->int {
    x:[16]f64;y:[16]f64;z:[6]f64;for i:0..<16 {x[i]=p[token*16+i];}
    rms:=td_norm(x[..],0,p,td_final_norm(s),16,y[..],0);#assert rms>0;td_matvec(p,td_unembed(s),6,16,y[..],0,z[..],0);return td_argmax(z[..]);
}
main :: () {
    s:=CD_SHAPE;n:=td_parameter_count(s);p:[3856]f64;#assert n==3856;seed:=td_init(s,CrSeed.{seed=113},p[..]);#assert seed.counter==3856;
    w:[4096]f64;dw:[4096]f64;#assert td_work_count(s)<=4096;grads:[3856]f64;total:[3856]f64;m:[3856]f64;v:[3856]f64;
    groups:=[1]AwGroup.[AwGroup.{first=0,count=3856,lr=0.01,beta1=0.9,beta2=0.99,eps=0.00000001,decay=0}];state:AwState;none:[1]f32;
    train:[24][8]int;held:[6][8]int;nt:=0;nh:=0;
    for a:0..<6 {for b:0..<6 {if a!=b {if cd_held_out(a,b) {cd_sequence(a,b,held[nh][..]);nh+=1;}else {cd_sequence(a,b,train[nt][..]);nt+=1;}}}}
    print("decoder: % parameters, % layers of 2-query/1-key-value-head attention; % training and % held-out pairs\n",n,s.layers,nt,nh);
    for step:0..<120 {
        loss:=0.0;for i:0..<n {total[i]=0;}
        for k:0..<24 {r:=td_gradients(s,p[..],train[k][..],w[..],dw[..],grads[..],false);#assert r.status==TdStatus.ok;loss+=r.loss/24;for i:0..<n {total[i]+=grads[i]/24;}}
        if step%30==0 {print("step %: train loss %\n",step,loss);}
        a:=aw_step(groups[..],AwPolicy.{},state,p[..],total[..],m[..],v[..],none[0..<0],false);#assert a.status==AwStatus.ok;state=a.state;
    }
    logits:[48]f64;held_loss:=0.0;for k:0..<6 {r:=td_forward(s,p[..],held[k][..],w[..],logits[..],false);#assert r.status==TdStatus.ok;held_loss+=r.loss/6;}
    print("held-out loss: %\n",held_loss);
    // Generate six tokens after each two-token prompt with the KV cache.
    copied:=0;head:=0;out:[6]f64;
    for k:0..<6 {tokens:=held[k];filled:=0;next:=tokens[0];generated:[8]int;generated[0]=tokens[0];generated[1]=tokens[1];
        for t:0..<7 {r:=td_decode(s,p[..],w[..],filled,generated[t],out[..],false);#assert r.status==TdStatus.ok;filled=r.written;next=td_argmax(out[..]);if t>=1 {generated[t+1]=next;}}
        same:=true;for t:0..<8 {if generated[t]!=tokens[t] {same=false;}}if same {copied+=1;}
        for t:1..<7 {if cd_head_only(s,p[..],tokens[t])==tokens[t+1] {head+=1;}}
        if k<2 {print("prompt % % -> % % % % % %\n",tokens[0],tokens[1],generated[2],generated[3],generated[4],generated[5],generated[6],generated[7]);}
    }
    print("held-out prompts continued exactly: % of 6\n",copied);
    print("head-only next-token accuracy on the same positions: % of 36\n",head);
    #assert copied==6 && head<36;
}
```

```text
decoder: 3856 parameters, 2 layers of 2-query/1-key-value-head attention; 24 training and 6 held-out pairs
step 0: train loss 2.0019668884972965
step 30: train loss 0.22580630853515607
step 60: train loss 0.20176667649377125
step 90: train loss 0.20023256428124206
held-out loss: 1.0845807076927156
prompt 0 1 -> 0 1 0 1 0 1
prompt 1 2 -> 1 2 1 2 1 2
held-out prompts continued exactly: 6 of 6
head-only next-token accuracy on the same positions: 3 of 36
```

The training loss settles near `ln(5)/7 = 0.23` nats: position 1 cannot be
predicted (any of five symbols may follow `a`) and every later position can.
The held-out loss is higher because that unpredictable position now carries a
pair the model never saw. Every held-out prompt continues exactly. The
vocabulary head alone, reading only the current token, gets few of the same
positions right: the blocks are doing the work.

```shell
coax/bin/coax run psionic/examples/causal-decoder.coax
psionic/check.sh --only decoder
```

The [transcript](../fixtures/causal-decoder.out) is exact for the named CPU
build, in debug and release. `check_decoder.sh` also compiles the library to
C and runs a native harness, plain and under ASan/UBSan, against the Python
goldens, with central differences for every parameter. Compiling the backward
pass (114) and training the whole decoder as a permanent regression (115)
come next.
