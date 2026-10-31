# 218. Read a bounded ONNX graph

Many small models reach us as ONNX files: an embedding table exported from
PyTorch, a classifier someone trained elsewhere. An ONNX file is a protobuf
message holding a graph of operators and the weights they read. The format is
large, with almost two hundred operators, external weight files, symbolic
shapes and local functions.

This chapter reads a small, fixed part of it and refuses the rest:
- **One operator set:** opset 17 of the default domain, and IR versions 7
  to 10.
- **Seven operators:** Gather, ReduceMean, Conv, Relu, Flatten, Gemm and
  Softmax, each with only the attribute values it implements. A Conv with
  strides of 2, for example, is refused, not run wrongly.
- **Weights in the file:** float tensors, as raw bytes or as `float_data`.
  Doubles, external data files and sparse tensors are refused.
- **Static shapes:** every dimension a number, at most rank 4 and 4,096
  elements a tensor.
- **Bounds:** at most 16 nodes, 32 values and 1 MiB of file.

An exporter often leaves weights anonymous (`onnx::Conv_5`). The reader names
each one from the node that uses it, so `/conv/Conv`'s weight becomes
`conv.weight`, as in the PyTorch module it came from. Every refusal names its
reason and, where there is one, the node.

```shell
coax/bin/coax run psionic/examples/onnx-read.coax
coax/bin/coax test psionic/lib/onnx.coax
```

```jai
embedding.onnx: 2345 bytes, IR 8, opset 17: 2 nodes, 1 weight, 592 floats of arena
  onnx::Gather_0 -> embed.weight f32 [64, 8]
  input_ids int64 [1, 8] -> sentence f32 [1, 8]
  ids 3 17 17 42 0 63 8 21 -> -0.035156 -0.136719 0.023438 -0.041016 -0.025391 0.166016 -0.064453 0.058594
  id -1 counts from the end, as 63 -> -0.035156 -0.136719 0.023438 -0.041016 -0.025391 0.166016 -0.064453 0.058594
  id 64: input at node 0
classifier.onnx: 1175 bytes, IR 8, opset 17: 5 nodes, 4 weights, 257 floats of arena
  onnx::Conv_5 -> conv.weight f32 [2, 1, 3, 3]
  onnx::Conv_6 -> conv.bias f32 [2]
  onnx::Gemm_7 -> fc.weight f32 [3, 32]
  onnx::Gemm_8 -> fc.bias f32 [3]
  pixels f32 [1, 1, 6, 6] -> probs f32 [1, 3]
  image 0 -> 0.715006 0.081689 0.203305
  image 1 -> 0.923834 0.017315 0.058851
refused:
  refuse_opset.onnx: opset
  refuse_ir.onnx: ir
  refuse_domain.onnx: opset
  refuse_function.onnx: field
  refuse_sparse.onnx: field
  refuse_op.onnx: op at node 1
  refuse_trans_a.onnx: attribute at node 3
  refuse_alpha.onnx: attribute at node 3
  refuse_pads.onnx: attribute at node 0
  refuse_unknown_attr.onnx: attribute at node 1
  refuse_double.onnx: dtype
  refuse_external.onnx: data
  refuse_short.onnx: data
  refuse_symbolic.onnx: shape
  refuse_order.onnx: order at node 1
  refuse_unused.onnx: unused
  refuse_shared.onnx: shared at node 3
  refuse_role.onnx: role at node 1
  refuse_wrong_output.onnx: output
  refuse_duplicate.onnx: name at node 2
  classifier.onnx cut at 600 bytes: wire
  classifier.onnx with 200 floats of arena: capacity
```

The [example](../examples/onnx-read.coax) reads two models and runs them in
under a second:
- **A static-embedding model:** a 64-row table read by Gather, then averaged
  by ReduceMean into one sentence vector.
- **A tiny image classifier:** Conv, Relu, Flatten, Gemm and Softmax over a
  6×6 image.

It then reads 19 files that each break one rule, a file cut short, and the
classifier with too little memory. All 21 files were written by `onnx` 1.17.0
from [`onnx_fixtures.py`](../tests/onnx_fixtures.py), which also checks them
with `onnx.checker` and runs the two models with `onnx`'s ReferenceEvaluator.

## A field at a time

A protobuf field is a key (its number and wire type) followed by a varint,
eight bytes, four bytes, or a length and that many bytes. The reader scans
those itself and refuses groups, unknown wire types, field 0, a varint past 64
bits and anything cut short as `wire`. Each message is then checked against
the fields this reader knows. A field it doesn't know is refused as `field`,
except doc strings and metadata, which are skipped.

An initializer is the strictest:

<!-- source: psionic/lib/onnx.coax initializer -->
```jai
// An initializer is a float tensor of at most rank 4 whose data is in the file,
// as raw little-endian bytes or as float_data, exactly as many values as its
// dimensions hold. Other element types are refused first; then external data
// and any other data field. Its values go into the arena.
ox_initializer :: (wire:[]u8,m:OxModel,arena:[]f32)->OxModel {
    r:=m;dims:=OxInts.{ok=true};dtype:=0;name:[]u8;raw:[]u8;has_raw:=false;floats:=0;elsewhere:=false;
    at:=r.arena;
    f:=ox_scan(wire,0);
    while !f.done {
        if f.error {return ox_fail(r,OxStatus.wire);}
        if f.number==1 {dims=ox_ints(f,dims);}
        else if f.number==2 && f.wt==0 {dtype=ox_small(f.raw);}
        else if f.number==8 && f.wt==2 {name=f.payload;}
        else if f.number==9 && f.wt==2 {raw=f.payload;has_raw=true;}
        else if f.number==4 && f.wt==5 {
            if at+floats==arena.count {return ox_fail(r,OxStatus.capacity);}
            arena[at+floats]=f32_from_bits(cast(u32)f.raw);floats+=1;
        }
        else if f.number==4 && f.wt==2 {
            if f.payload.count%4!=0 {return ox_fail(r,OxStatus.data);}
            if at+floats+f.payload.count/4>arena.count {return ox_fail(r,OxStatus.capacity);}
            for i:0..<f.payload.count/4 {arena[at+floats+i]=ox_word(f.payload,4*i);}
            floats+=f.payload.count/4;
        }
        else if f.number!=12 && (f.number!=14 || f.raw!=0) {elsewhere=true;}
        f=ox_scan(wire,f.next);
    }
    if !dims.ok {return ox_fail(r,OxStatus.shape);}
    if dtype!=OX_FLOAT {return ox_fail(r,OxStatus.dtype);}
    if elsewhere {return ox_fail(r,OxStatus.data);}
    if name.count==0 || ox_find(r,name)!=-1 {return ox_fail(r,OxStatus.name);}
    shape:=OxShape.{rank=dims.count};
    for i:0..<dims.count {shape.dims[i]=cast(int)dims.values[i];}
    count:=ox_elements(shape);if count==0 {return ox_fail(r,OxStatus.shape);}
    if has_raw==(floats!=0) || (has_raw && raw.count!=4*count) || (!has_raw && floats!=count) {return ox_fail(r,OxStatus.data);}
    if at+count>arena.count {return ox_fail(r,OxStatus.capacity);}
    if has_raw {for i:0..<count {arena[at+i]=ox_word(raw,4*i);}}
    if r.value_count==OX_VALUES_MAX {return ox_fail(r,OxStatus.capacity);}
    r.values[r.value_count]=OxValue.{name=name,kind=OxKind.weight,shape=shape,at=at,count=count};
    r.value_count+=1;r.arena=at+count;
    return r;
}
```

The element type is checked first, so a double tensor is refused as `dtype`
before its data are looked at. Raw bytes and `float_data` can't both be set,
and the value count must match the dimensions exactly. The values go into one
arena of floats that the caller provides. A model that needs more arena than
it is given is refused as `capacity`, never truncated.

## Names from module paths

<!-- source: psionic/lib/onnx.coax names -->
```jai
// A weight's name comes from the node that uses it. A PyTorch export names a
// node by its module path ("/encoder/fc/Gemm"); the path less its first slash
// and its last part becomes the stem ("encoder.fc"). A node with no module
// path ("/Flatten", "" or "fc") is named by its operator and position
// ("gemm3"). The first weight slot is the weight, the second the bias.
ox_stem :: (node:[]u8,op:OxOp,index:int,out:[]u8)->int {
    slashes:=0;last:int=---;
    for c,k:node {if c=='/' {slashes+=1;last=k;}}
    n:=0;
    if slashes>1 && node[0]=='/' {
        for k:1..<last {if n<out.count {out[n]=ifx node[k]=='/' then '.' else node[k];} n+=1;}
        return n;
    }
    stem:=OX_STEMS[cast(int)op];
    for k:0..<stem.count {out[n]=stem[k];n+=1;}
    if index>=10 {out[n]='1';n+=1;}
    out[n]=cast(u8)('0'+index%10);
    return n+1;
}
ox_name_weight :: (m:OxModel,node:[]u8,op:OxOp,index:int,slot:int,value:int)->OxModel {
    r:=m;t:=OxTensor.{value=value};
    n:=ox_stem(node,op,index,t.name[..]);
    suffix:=ifx slot==2 then ".bias" else ".weight";
    if n==0 || n+suffix.count>t.name.count {return ox_refuse(r,OxStatus.name,index);}
    for k:0..<suffix.count {t.name[n+k]=suffix[k];}
    t.length=n+suffix.count;
    for k:0..<r.tensor_count {x:=r.tensors[k];if ox_same(x.name[0..<x.length],t.name[0..<t.length]) {return ox_refuse(r,OxStatus.name,index);}}
    r.tensors[r.tensor_count]=t;r.tensor_count+=1;
    return r;
}
```

A node with a module path (`/encoder/fc/Gemm`) gives its weights the path's
middle as a stem (`encoder.fc.weight`, `encoder.fc.bias`). A node with no
path, such as `/Flatten`, or an empty name, is named by operator and position
(`gemm3.weight`). Two weights that would get the same name are refused as
`name`.

## The graph, in order

<!-- source: psionic/lib/onnx.coax graph -->
```jai
// The graph's fields are checked in one pass, then used in four steps:
// initializers, the input (skipping names an initializer already holds, as
// older exports list them), nodes in file order, the output. A node may only
// read a value defined before it, so a graph out of topological order is
// refused.
OX_FIELDS_MAX :: 64;
ox_graph :: (wire:[]u8,m:OxModel,arena:[]f32)->OxModel {
    r:=m;at:[4][OX_FIELDS_MAX][]u8;count:[4]int;
    f:=ox_scan(wire,0);
    while !f.done {
        if f.error {return ox_fail(r,OxStatus.wire);}
        n:=f.number;
        step:=ifx n==5 then 0 else ifx n==11 then 1 else ifx n==1 then 2 else ifx n==12 then 3 else -1;
        if step==-1 && n!=2 && n!=10 && n!=13 && n!=16 {return ox_fail(r,OxStatus.field);}
        if step!=-1 {
            if f.wt!=2 {return ox_fail(r,OxStatus.wire);}
            if count[step]==OX_FIELDS_MAX {return ox_fail(r,OxStatus.capacity);}
            at[step][count[step]]=f.payload;count[step]+=1;
        }
        f=ox_scan(wire,f.next);
    }
    for k:0..<count[0] {r=ox_initializer(at[0][k],r,arena);if r.status!=OxStatus.ok {return r;}}
    for k:0..<count[1] {
        d:=ox_declaration(at[1][k]);
        if d.status!=OxStatus.ok {return ox_fail(r,d.status);}
        if ox_find(r,d.name)!=-1 {continue;}
        if r.input!=-1 {return ox_fail(r,OxStatus.input);}
        if d.elem!=OX_FLOAT && d.elem!=OX_INT64 {return ox_fail(r,OxStatus.dtype);}
        n:=ox_elements(d.shape);
        if n==0 || r.value_count==OX_VALUES_MAX || r.arena+n>arena.count {return ox_fail(r,OxStatus.capacity);}
        r.input=r.value_count;
        r.values[r.value_count]=OxValue.{name=d.name,kind=OxKind.input,integer=d.elem==OX_INT64,shape=d.shape,at=r.arena,count=n};
        r.value_count+=1;r.arena+=n;
    }
    if r.input==-1 {return ox_fail(r,OxStatus.input);}
    for k:0..<count[2] {
        if k==OX_NODES_MAX {return ox_refuse(r,OxStatus.capacity,k);}
        r=ox_node(at[2][k],r,k,arena.count);if r.status!=OxStatus.ok {return r;}
    }
    for k:0..<count[3] {
        d:=ox_declaration(at[3][k]);
        if d.status!=OxStatus.ok {return ox_fail(r,d.status);}
        v:=ox_find(r,d.name);
        if r.output!=-1 || v==-1 {return ox_fail(r,OxStatus.output);}
        if r.values[v].kind!=OxKind.computed || d.elem!=OX_FLOAT || !ox_shape_same(d.shape,r.values[v].shape) {return ox_fail(r,OxStatus.output);}
        r.output=v;
    }
    if r.output==-1 {return ox_fail(r,OxStatus.output);}
    for v:r.values {if v.kind==OxKind.weight && v.uses==0 {return ox_fail(r,OxStatus.unused);}}
    return r;
}
```

The reader works in file order:
- A node may only read a value defined before it; otherwise it is refused as
  `order`.
- A weight must sit in the slot that expects one (`role`).
- A weight read by two nodes is refused as `shared`, since one weight can't
  have two names.
- Every value must be used (`unused`).
- The graph's output must be the last node's.

Each node's output shape is inferred from its inputs and checked against what
the operator allows. A Conv kernel larger than its input, for instance, is
refused as `shape`.

## Running it

<!-- source: psionic/lib/onnx.coax run -->
```jai
// The admitted graph on the CPU: binary32 values, sums in a fixed order,
// Softmax in binary64. An index outside the table is refused at run time.
ox_run :: (m:OxModel,arena:[]f32,ids:[]s64,pixels:[]f32)->OxRun #export_c {
    if m.status!=OxStatus.ok || arena.count<m.arena {return OxRun.{status=OxStatus.capacity,node=-1};}
    x:=m.values[m.input];
    if x.integer {
        if ids.count!=x.count {return OxRun.{status=OxStatus.input,node=-1};}
        for i:0..<x.count {if ids[i]<-OX_ELEMENTS_MAX || ids[i]>OX_ELEMENTS_MAX {return OxRun.{status=OxStatus.input,node=-1};} arena[x.at+i]=f32(ids[i]);}
    } else {
        if pixels.count!=x.count {return OxRun.{status=OxStatus.input,node=-1};}
        for i:0..<x.count {arena[x.at+i]=pixels[i];}
    }
    for k:0..<m.node_count {
        n:=m.nodes[k];a:=m.values[n.inputs[0]];y:=m.values[n.output];
        if n.op==OxOp.gather {
            b:=m.values[n.inputs[1]];rows:=a.shape.dims[0];d:=a.shape.dims[1];
            for i:0..<b.count {
                id:=cast(int)arena[b.at+i];if id<0 {id+=rows;}
                if id<0 || id>=rows {return OxRun.{status=OxStatus.input,node=k};}
                for j:0..<d {arena[y.at+i*d+j]=arena[a.at+id*d+j];}
            }
        } else if n.op==OxOp.reduce_mean {
            // Outer, reduced and inner extents around the axis.
            outer:=1;inner:=1;span:=a.shape.dims[n.axis];
            for q:0..<n.axis {outer*=a.shape.dims[q];}
            for q:n.axis+1..<a.shape.rank {inner*=a.shape.dims[q];}
            for o:0..<outer {for i:0..<inner {
                total:f32=0;
                for s:0..<span {total+=arena[a.at+(o*span+s)*inner+i];}
                arena[y.at+o*inner+i]=total/f32(span);
            }}
        } else if n.op==OxOp.conv {
            w:=m.values[n.inputs[1]];c:=a.shape.dims[1];h:=a.shape.dims[2];wd:=a.shape.dims[3];
            kh:=w.shape.dims[2];kw:=w.shape.dims[3];oh:=y.shape.dims[2];ow:=y.shape.dims[3];maps:=y.shape.dims[1];
            for b:0..<a.shape.dims[0] {for f:0..<maps {for i:0..<oh {for j:0..<ow {
                total:f32=ifx n.arity==3 then arena[m.values[n.inputs[2]].at+f] else 0;
                for ch:0..<c {for u:0..<kh {for v:0..<kw {
                    total+=arena[a.at+((b*c+ch)*h+i+u)*wd+j+v]*arena[w.at+((f*c+ch)*kh+u)*kw+v];
                }}}
                arena[y.at+((b*maps+f)*oh+i)*ow+j]=total;
            }}}}
        } else if n.op==OxOp.relu {
            for i:0..<a.count {v:=arena[a.at+i];arena[y.at+i]=ifx v>0 then v else 0;}
        } else if n.op==OxOp.flatten {
            for i:0..<a.count {arena[y.at+i]=arena[a.at+i];}
        } else if n.op==OxOp.gemm {
            w:=m.values[n.inputs[1]];rows:=a.shape.dims[0];depth:=a.shape.dims[1];cols:=y.shape.dims[1];
            for i:0..<rows {for j:0..<cols {
                total:f32=0;
                for q:0..<depth {total+=arena[a.at+i*depth+q]*arena[w.at+(ifx n.trans_b then j*depth+q else q*cols+j)];}
                if n.arity==3 {total+=arena[m.values[n.inputs[2]].at+j];}
                arena[y.at+i*cols+j]=total;
            }}
        } else {
            width:=a.shape.dims[a.shape.rank-1];
            for o:0..<a.count/width {
                top:=f64(arena[a.at+o*width]);
                for j:1..<width {v:=f64(arena[a.at+o*width+j]);if v>top {top=v;}}
                total:=0.0;
                for j:0..<width {total+=exp(f64(arena[a.at+o*width+j])-top);}
                for j:0..<width {arena[y.at+o*width+j]=cast(f32)(exp(f64(arena[a.at+o*width+j])-top)/total);}
            }
        }
    }
    return OxRun.{node=-1};
}
```

Values are binary32, and sums run in a fixed order. Softmax subtracts each
row's largest value, then works in binary64, so a logit of 800 gives 1 and 0,
not an overflow. Gather follows ONNX, where a negative id counts from the end
of the table. An id outside the table is refused at run time, with the node
that read it.

## How it is checked

- **22 Coax checks:** the profile's bounds at their edges, and each
  refusal reached by its own hand-made bytes.
- **A case table of 123 ONNX files**, from `onnx_fixtures.py`. In 13 of
  them the reader's outputs must come within 1e-6 of `onnx`'s
  ReferenceEvaluator. The other 110 must be refused with the stated
  reason and node.
- **[`onnx_oracle.py`](../tests/onnx_oracle.py):** a separate reader and
  evaluator in Python's standard library. It replays the same table,
  emulating binary32, and rebuilds the example's output line for line.
- **Mutation:** 1,509 mutants, all killed but one. The survivor turns
  Softmax's `v>top` into `v>=top`. The two differ only when a value equals
  the running maximum, and then replace it with an equal value, so the
  mutant can't be told apart.

[Chapter 220](PLAN.md) puts a small encoder read this way behind a typed
decision. 219 writes the same subset out.
