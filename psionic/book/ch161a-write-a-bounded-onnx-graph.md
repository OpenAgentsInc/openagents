# 161a. Write a bounded ONNX graph

[Chapter 161](ch161-read-a-bounded-onnx-graph.md) reads a small part of ONNX
and refuses the rest. This chapter writes the same part out, so a model
trained in Psionic can leave as an inert file that other runtimes load.

The writer takes a chain of Gemm, Relu and Softmax nodes over one float
input, with its weights, and writes:
- **The bytes `onnx` itself would write.** Fields go in field-number order,
  and every length is the shortest varint. The tests compare whole files
  with ones built by `onnx`'s own helpers.
- **Names chapter 161 gives back.** A Gemm is `/stem/Gemm`, and its weights
  are `stem.weight` and `stem.bias`.
- **Only files chapter 161 admits.** Each file is read back before it is
  returned. A graph the reader refuses is refused here, with the reader's
  reason and node.

The first models out are [chapter 70](ch070-learning-with-arrays.md)'s two
affine models. Chapter 70 trains them on scaled features. The export folds
the scaling into the weights, so the files read raw features. Chapter 70's
hidden-layer model is refused: it computes tanh, which has no operator in the
subset.

```shell
coax/bin/coax run psionic/examples/onnx-write.coax
coax/bin/coax test psionic/lib/onnx-write.coax
```

```jai
chapter 70's deadline requests: 1440 training rows, 480 test rows, 4 raw features
trained in binary64: logistic test log loss 0.1593, line test squared error 3.8812 s^2
deadline.onnx: Gemm, then Softmax over [0, z]
  287 bytes, 2 nodes; deadline.weight, deadline.bias read back by chapter 161 bit for bit
  480 test rows: P(on time) within 5.82e-07 of the binary64 model; decisions at 0.5 agree on 480 of 480
latency.onnx: Gemm alone
  224 bytes, 1 node; latency.weight, latency.bias read back by chapter 161 bit for bit
  480 test rows: seconds within 1.456e-06 of the binary64 model
chapter 70's MLP: op, tanh is outside the subset
sample test rows:
  row 32: features 6348.0 14.0 1.0 2.5901588952142 -> deadline 0.9795451164245605 0.020454896613955498
  row 32: features 6348.0 14.0 1.0 2.5901588952142 -> latency 11.413291931152344
  row 632: features 6089.0 14.0 1.0 7.18334117587656 -> deadline 0.21108423173427582 0.788915753364563
  row 632: features 6089.0 14.0 1.0 7.18334117587656 -> latency 8.834364891052246
  row 1232: features 5448.0 3.0 1.0 7.614422908391288 -> deadline 0.005047868471592665 0.9949521422386169
  row 1232: features 5448.0 3.0 1.0 7.614422908391288 -> latency 6.037778377532959
  row 1832: features 3352.0 1.0 0.0 5.517786917359369 -> deadline 8.724552458261314e-08 0.9999999403953552
  row 1832: features 3352.0 1.0 0.0 5.517786917359369 -> latency 0.6946170926094055
deadline.onnx, in hex:
  08081207 7073696f 6e69633a 8b020a44 0a086665 61747572 65730a0f 64656164
  6c696e65 2e776569 6768740a 0d646561 646c696e 652e6269 61731202 74311a0e
  2f646561 646c696e 652f4765 6d6d2204 47656d6d 0a2b0a02 74311205 70726f62
  731a082f 536f6674 6d617822 07536f66 746d6178 2a0b0a04 61786973 1801a001
  02120770 73696f6e 69632a39 08040802 1001420f 64656164 6c696e65 2e776569
  6768744a 20000000 007d7df6 ba000000 00dcda57 be000000 00680c0c c1000000
  007afb82 3f2a1d08 02100142 0d646561 646c696e 652e6269 61734a08 00000000
  35fb8841 5a1a0a08 66656174 75726573 120e0a0c 08011208 0a020801 0a020804
  62170a05 70726f62 73120e0a 0c080112 080a0208 010a0208 0242040a 001011
latency.onnx, in hex:
  08081207 7073696f 6e69633a cc010a46 0a086665 61747572 65730a0e 6c617465
  6e63792e 77656967 68740a0c 6c617465 6e63792e 62696173 12077365 636f6e64
  731a0d2f 6c617465 6e63792f 47656d6d 22044765 6d6d1207 7073696f 6e69632a
  28080408 01100142 0e6c6174 656e6379 2e776569 6768744a 10d1e87f 3a15c335
  3e0a1f80 40dba401 bf2a1808 01100142 0c6c6174 656e6379 2e626961 734a0454
  69203d5a 1a0a0866 65617475 72657312 0e0a0c08 0112080a 0208010a 02080462
  190a0773 65636f6e 6473120e 0a0c0801 12080a02 08010a02 08014204 0a001011
```

The [example](../examples/onnx-write.coax) runs in about a second:
- It trains both models as chapter 70 does, then exports them.
- It reads each file back with chapter 161, checking every name and every
  weight bit.
- It runs all 480 test rows through the files and compares them with the
  binary64 models.

The files differ from those models by under 1.5e-6, from rounding to
binary32 once, and every decision at 0.5 is the same.

## A message is written before its length is known

<!-- source: psionic/lib/onnx-write.coax wire -->
```jai
// Protobuf on the way out: a key is the field number times 8 plus the wire
// type; a nested message is written after a five-byte gap, then moved back
// once its length is known, so every length is the shortest varint, as
// onnx's own serializer writes it.
ow_var :: (out:[]u8,at:int,v:u64)->int {
    p:=at;x:=v;
    while x>=128 {out[p]=cast,trunc(u8)(x|128);x>>=7;p+=1;}
    out[p]=cast(u8)x;return p+1;
}
ow_int :: (out:[]u8,at:int,number:u64,v:u64)->int {return ow_var(out,ow_var(out,at,number*8),v);}
ow_text :: (out:[]u8,at:int,number:u64,text:string)->int {
    p:=ow_var(out,ow_var(out,at,number*8+2),cast(u64)text.count);
    for c,i:text {out[p+i]=c;}
    return p+text.count;
}
ow_parts :: (out:[]u8,at:int,number:u64,a:string,b:string,c:string)->int {
    p:=ow_var(out,ow_var(out,at,number*8+2),cast(u64)(a.count+b.count+c.count));
    for x:a {out[p]=x;p+=1;}
    for x:b {out[p]=x;p+=1;}
    for x:c {out[p]=x;p+=1;}
    return p;
}
ow_open :: (out:[]u8,at:int,number:u64)->int {return ow_var(out,at,number*8+2)+5;}
ow_close :: (out:[]u8,body:int,end:int)->int {
    p:=ow_var(out,body-5,cast(u64)(end-body));
    for i:0..<end-body {out[p+i]=out[body+i];}
    return p+end-body;
}
```

A nested protobuf message starts with its length. The writer leaves a
five-byte gap, writes the body, then writes the length and moves the body
back to meet it. That costs a copy per message. In exchange, each piece of
the file is written once, in order, with no size pass first.

## Tensors and their types

<!-- source: psionic/lib/onnx-write.coax pieces -->
```jai
// A graph input or output: a float tensor with static dimensions. An
// initializer: dimensions, FLOAT, the name, then the values as raw bytes.
ow_value :: (out:[]u8,at:int,number:u64,name:string,shape:OxShape)->int {
    v:=ow_open(out,at,number);p:=ow_text(out,v,1,name);
    t:=ow_open(out,p,2);tensor:=ow_open(out,t,1);p=ow_int(out,tensor,1,OX_FLOAT);
    s:=ow_open(out,p,2);p=s;
    for i:0..<shape.rank {d:=ow_open(out,p,1);p=ow_close(out,d,ow_int(out,d,1,cast(u64)shape.dims[i]));}
    p=ow_close(out,s,p);p=ow_close(out,tensor,p);p=ow_close(out,t,p);
    return ow_close(out,v,p);
}
ow_tensor :: (out:[]u8,at:int,stem:string,suffix:string,shape:OxShape,values:[]f32)->int {
    t:=ow_open(out,at,5);p:=t;
    for i:0..<shape.rank {p=ow_int(out,p,1,cast(u64)shape.dims[i]);}
    p=ow_int(out,p,2,OX_FLOAT);p=ow_parts(out,p,8,stem,suffix,"");
    p=ow_var(out,ow_var(out,p,9*8+2),cast(u64)(4*values.count));
    for v:values {bits:=f32_bits(v);for k:0..<4 {out[p]=cast,trunc(u8)(bits>>cast(u32)(8*k));p+=1;}}
    return ow_close(out,t,p);
}
```

A weight is written as raw little-endian binary32, as `numpy_helper` writes
one. The graph input and output declare their element type and every
dimension, because chapter 161 refuses symbolic shapes.

## The graph

<!-- source: psionic/lib/onnx-write.coax write -->
```jai
// A node reads the previous node's output (the graph input first); the last
// writes the graph output and the others "t1", "t2", ... A Gemm is named
// "/stem/Gemm" and its weights "stem.weight" and "stem.bias": with no slash in
// the stem, those are the names chapter 161 gives them back. The file is
// staged, read back, and copied out only if 161 admits it.
ow_check :: (g:OwGraph,values:[]f32)->OwResult {
    if g.node_count>OX_NODES_MAX || g.weight_count>OX_VALUES_MAX {return OwResult.{status=OwStatus.capacity,node=-1};}
    if g.node_count==0 {return OwResult.{status=OwStatus.shape,node=-1};}
    for k:0..<g.node_count {n:=g.nodes[k];
        if n.op!=OxOp.gemm && n.op!=OxOp.relu && n.op!=OxOp.softmax {return OwResult.{status=OwStatus.op,node=k};}
        if n.op!=OxOp.gemm {continue;}
        if n.stem.count==0 {return OwResult.{status=OwStatus.name,node=k};}
        for c:n.stem {if c=='/' {return OwResult.{status=OwStatus.name,node=k};}}
        if n.weight<0 || n.weight>=g.weight_count || n.bias<-1 || n.bias>=g.weight_count {return OwResult.{status=OwStatus.shape,node=k};}
        for slot:[2]int.[n.weight,n.bias] {
            if slot==-1 {continue;}
            w:=g.weights[slot];count:=ox_elements(w.shape);
            if count==0 || w.at<0 || w.at+count>values.count {return OwResult.{status=OwStatus.shape,node=k};}
        }
    }
    return OwResult.{};
}
ow_name :: (out:[]u8,at:int,number:u64,g:OwGraph,k:int)->int {
    if k==-1 {return ow_text(out,at,number,g.input);}
    if k==g.node_count-1 {return ow_text(out,at,number,g.output);}
    digit:=[1]u8.[cast(u8)(49+k)];
    if k<9 {return ow_parts(out,at,number,"t",cast(string)digit[..],"");}
    digit[0]=cast(u8)(39+k);return ow_parts(out,at,number,"t1",cast(string)digit[..],"");
}
ow_write :: (g:OwGraph,values:[]f32,out:[]u8)->OwResult {
    r:=ow_check(g,values);if r.status!=OwStatus.ok {return r;}
    file:[OX_BYTES_MAX]u8;
    p:=ow_int(file[..],0,1,OW_IR);p=ow_text(file[..],p,2,"psionic");
    graph:=ow_open(file[..],p,7);p=graph;
    input:=OxShape.{rank=2};input.dims[0]=g.rows;input.dims[1]=g.features;shape:=input;
    for k:0..<g.node_count {n:=g.nodes[k];
        node:=ow_open(file[..],p,1);p=ow_name(file[..],node,1,g,k-1);
        if n.op==OxOp.gemm {
            p=ow_parts(file[..],p,1,n.stem,".weight","");
            if n.bias!=-1 {p=ow_parts(file[..],p,1,n.stem,".bias","");}
            w:=g.weights[n.weight].shape;shape.dims[1]=ifx n.trans_b then w.dims[0] else w.dims[1];
        }
        p=ow_name(file[..],p,2,g,k);
        if n.op==OxOp.gemm {p=ow_parts(file[..],p,3,"/",n.stem,"/Gemm");}
        else {p=ow_text(file[..],p,3,ifx n.op==OxOp.relu then "/Relu" else "/Softmax");}
        p=ow_text(file[..],p,4,OX_OP_NAMES[cast(int)n.op]);
        if n.trans_b || n.op==OxOp.softmax {
            a:=ow_open(file[..],p,5);
            q:=ow_text(file[..],a,1,ifx n.op==OxOp.softmax then "axis" else "transB");
            q=ow_int(file[..],q,3,ifx n.op==OxOp.softmax then cast,trunc(u64)n.axis else 1);
            p=ow_close(file[..],a,ow_int(file[..],q,20,OX_INT));
        }
        p=ow_close(file[..],node,p);
    }
    p=ow_text(file[..],p,2,"psionic");
    for k:0..<g.node_count {n:=g.nodes[k];
        if n.op!=OxOp.gemm {continue;}
        w:=g.weights[n.weight];p=ow_tensor(file[..],p,n.stem,".weight",w.shape,values[w.at..<w.at+ox_elements(w.shape)]);
        if n.bias!=-1 {b:=g.weights[n.bias];p=ow_tensor(file[..],p,n.stem,".bias",b.shape,values[b.at..<b.at+ox_elements(b.shape)]);}
    }
    p=ow_value(file[..],p,11,g.input,input);
    p=ow_value(file[..],p,12,g.output,shape);
    p=ow_close(file[..],graph,p);
    opset:=ow_open(file[..],p,8);p=ow_close(file[..],opset,ow_int(file[..],ow_text(file[..],opset,1,""),2,OX_OPSET));
    arena:[OW_ARENA]f32;m:=ox_read(file[0..<p],arena[..]);
    if m.status!=OxStatus.ok {return OwResult.{status=OwStatus.reread,node=m.node,read=m.status};}
    if out.count<p {return OwResult.{status=OwStatus.capacity,node=-1};}
    for i:0..<p {out[i]=file[i];}
    return OwResult.{node=-1,size=p};
}
```

The writer checks the few things it needs before it can write:
- the operators (`op`);
- each Gemm's weights, which must be in range with a non-empty shape
  (`shape`);
- each Gemm's stem, which must be non-empty and contain no slash (`name`);
- the bounds (`capacity`).

Everything else is left to chapter 161. Shapes that don't chain, a stem too
long for 161's 64-byte names, an input and output with the same name, and a
Softmax axis that isn't the last all come back as `reread`, with 161's
reason and node.

A weight given to two nodes is written once under each node's name, so the
file never shares a weight.

## Folding the scaling into the weights

<!-- source: psionic/lib/onnx-write.coax fold -->
```jai
// Chapter 70's affine models read scaled features, (x - mean) / sd. Folding
// the scaling into the weights lets the exported graph read raw features:
// w / sd for each weight, and b - sum(w mean / sd) for the bias, in binary64
// with chapter 3's accumulator, then rounded once to binary32. The logistic
// model becomes two columns, 0 and z, so a two-class Softmax gives sigmoid(z)
// as its second output.
ow_from_tabular :: (m:TbModel,params:[]f64,mean:[]f64,sd:[]f64,values:[]f32,stem:string)->OwFold {
    if m.hidden!=0 {return OwFold.{status=OwStatus.op};}
    d:=m.features;
    if !tb_model_ok(m) || params.count<d+1 || mean.count<d || sd.count<d {return OwFold.{status=OwStatus.shape};}
    cols:=ifx m.logistic then 2 else 1;
    if values.count<cols*(d+1) {return OwFold.{status=OwStatus.capacity};}
    folded:[TB_FEATURES_MAX+1]f64;
    a:PsionicAccumulator;a=psionic_ops_accumulate(a,params[d]);
    for j:0..<d {folded[j]=params[j]/sd[j];a=psionic_ops_accumulate(a,-(params[j]*mean[j]/sd[j]));}
    folded[d]=psionic_ops_total(a);
    for v:folded[0..<d+1] {if !(abs(v)<=OW_F32_MAX) {return OwFold.{status=OwStatus.number};}}
    for j:0..<d+1 {
        if m.logistic {values[j*2]=0;values[j*2+1]=cast(f32)folded[j];}
        else {values[j]=cast(f32)folded[j];}
    }
    g:=OwGraph.{input="features",rows=1,features=d,output=ifx m.logistic then "probs" else "seconds",node_count=cols,weight_count=2};
    g.nodes[0]=OwNode.{op=OxOp.gemm,stem=stem,weight=0,bias=1};
    g.nodes[1]=OwNode.{op=OxOp.softmax,axis=1};
    g.weights[0]=OwWeight.{shape=OxShape.{rank=2,dims=[4]int.[d,cols,0,0]}};
    g.weights[1]=OwWeight.{shape=OxShape.{rank=1,dims=[4]int.[cols,0,0,0]},at=cols*d};
    return OwFold.{graph=g,count=cols*(d+1)};
}
```

Chapter 70's logistic model gives the probability of finishing on time as
sigmoid(z). The subset has no Sigmoid, but a two-class Softmax over [0, z]
gives exactly 1 / (1 + e^-z) as its second output, so the file needs nothing
new. The latency line is a Gemm with one column.

The fold works in binary64 and rounds each weight to binary32 once. A
weight past binary32's largest value is refused as `number`, not written as
infinity.

## How it is checked

- **8 Coax checks:**
  - every piece's bytes;
  - a two-layer network and a 16-node chain, each equal byte for byte to
    the file `onnx` 1.17.0 builds for the same graph;
  - each refusal, at its edge;
  - the fold, with chapter 70's widest model and binary32's range limit.
- **[`onnx_write_check.py`](../tests/onnx_write_check.py)** runs on the
  example's two files. Each must:
  - pass `onnx.checker`;
  - equal what `onnx`'s helpers build from the same weights;
  - give the example's printed outputs on its sample rows under `onnx`'s
    ReferenceEvaluator.

  This needs `onnx` and numpy, so it is run by hand, not by `check.sh`.
- **[`onnx_write_oracle.py`](../tests/onnx_write_oracle.py)** uses only the
  standard library. It:
  - trains the models with chapter 70's oracle;
  - folds them;
  - encodes both files with its own protobuf writer;
  - runs them in emulated binary32;
  - rebuilds the example's output line for line.
- **Mutation:** 2,820 of 2,821 mutants killed, across this writer and chapter 161's
  reader. The one survivor is 161's Softmax tie, which can't be told apart.

Not in the subset yet:
- writing Gather, ReduceMean, Conv or Flatten, which 161 can read;
- branches, or more than one input;
- any activation 161 doesn't run, which includes the tanh that keeps
  chapter 70's hidden layer from leaving.
