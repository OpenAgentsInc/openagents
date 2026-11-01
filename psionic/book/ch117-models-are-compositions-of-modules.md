# 117. Models are compositions of modules

> **Status: partly implemented.** The module layer runs on the CPU: named
> modules, parameters and buffers, train and eval modes, initialization
> rules, freezing, shared parameters, and state dictionaries saved and loaded
> by name. A convolutional network and a full transformer block cannot be
> built yet, because chapter 38's tensor IR has no convolution, softmax or
> exponential, and one program holds at most 32 nodes.

Up to here every network in this book was written by hand: [chapter 106](ch106-differentiate-any-tensor-program.md)'s
two-layer network is seventeen numbered nodes and a list of input offsets.
That works for one network. It does not scale to many, and it gives the
weights no names, so nothing outside the program can say which number is
which. This chapter adds the layer that frameworks call modules:

- a **module** is a named part of a model, such as `fc1`, with children of
  its own;
- a **parameter** is a named tensor that training changes, such as
  `fc1.weight`;
- a **buffer** is a named tensor that is saved with the model but never
  trained, such as a fixed scale; a **transient** is rebuilt before every
  run and never saved, such as a dropout mask;
- the **state dictionary** is every parameter and buffer by name, written
  to and read from [chapter 8](ch008-models-travel-between-frameworks.md)'s
  named-tensor file.

The module layer adds no arithmetic. Each constructor appends input nodes
and operations to one of [chapter 38](ch038-a-tensor-program-becomes-an-execution-plan.md)'s
tensor programs and records a name, a shape and an offset for every input.
Chapter 106's tape then runs the program and differentiates it, exactly as
if you had written the nodes by hand. The checks prove that: the composed
model and the same program written node by node give the same loss and the
same gradients, bit for bit.

The runnable profile is `module-f32-v1`: f32 values in one input buffer
that you own, at most 16 modules, at most 32 program nodes (so at most 32
named entries), at most 16 saved tensors, and names of at most 63 bytes.
The code is [`module.coax`](../lib/module.coax), and the example that
prints the output below is [`module-block.coax`](../examples/module-block.coax).

```shell
coax/bin/coax run psionic/examples/module-block.coax
coax/bin/coax test psionic/lib/module.coax
```

```text
model: status ok, 6 modules, 11 entries, 27 program nodes, 153 input floats, workspace 96 floats
  x  data [4,4] at 0
  target  data [4,4] at 16
  norm.weight  parameter [4] at 32, constant 1.0
  norm.bias  parameter [4] at 36, constant 0.0
  fc1.weight  parameter [4,8] at 40, uniform +- 0.5
  fc1.bias  parameter [8] at 72, uniform +- 0.5
  drop.mask  transient [4,8] at 80
  fc2.weight  parameter [8,4] at 112, uniform +- 0.353553
  fc2.bias  parameter [4] at 144, uniform +- 0.353553
  scale  buffer [4] at 148, constant 0.5
  loss.neg  transient [1] at 152
trainable floats 84, saved floats 88

composed against hand-wired, training mode, mask from step 1:
  loss 9.490625 and 9.490625, same bits: true
  gradient of norm.weight: norm 1.471426, same bits: true
  gradient of norm.bias: norm 0.892046, same bits: true
  gradient of fc1.weight: norm 4.88874, same bits: true
  gradient of fc1.bias: norm 2.323348, same bits: true
  gradient of fc2.weight: norm 9.221085, same bits: true
  gradient of fc2.bias: norm 1.961579, same bits: true

training with norm frozen, dropout 128/256, learning rate 0.02:
  step 0: eval loss 9.122487
  step 100: eval loss 1.054721
  step 200: eval loss 1.211376
  step 300: eval loss 0.850827
  step 400: eval loss 0.903775
  norm.weight and norm.bias unchanged: true

state dictionary: ok, 7 tensors, 872 bytes
loaded: ok, 7 tensors; eval loss 0.903775 before and 0.903775 after, same bits: true

refused loads:
  model with an extra gate: missing, entry gate
  model without scale: unexpected, 1 unexpected, file tensor scale
  the same, not requiring every key: ok, 6 loaded, 1 unexpected
  fc1 16 wide: mismatch, entry fc1.weight
  damaged header: format
  bad names: name name name
```

## Entries and names

A model is one value of type `MdlModel`. It holds the tensor program, the
module tree, and a list of entries. An entry is one named input of the
program: its kind, its shape, where it lives in the input buffer, and how to
fill it.

<!-- source: psionic/lib/module.coax types -->
```jai
MdlStatus :: enum u8 {ok;name;capacity;shape;program;missing;unexpected;mismatch;format;state;number;}
// parameter: trained and saved. buffer: saved, never trained (a fixed
// scale, running statistics). transient: rebuilt by mdl_refresh before each
// run and never saved (a dropout mask, a constant). data: the batch.
MdlKind :: enum u8 {parameter;buffer;transient;data;}
// constant fills with value; uniform draws from [-value, value) with the
// counter generator, one counter per element.
MdlInit :: enum u8 {constant;uniform;}
MdlName :: struct {count:int;bytes:[64]u8;}
MdlEntry :: struct {
    name:MdlName;kind:MdlKind;owner:int;node:int;shape:TvShape;base:int;size:int;
    init:MdlInit;value:f32;frozen:bool;mask:bool;drop:int;
}
MdlModule :: struct {name:MdlName;parent:int;}
// A model under construction. Each constructor returns the model with the
// node it made last in node and the module it made in child. A refusal is
// sticky: the first failing call sets status and later calls change nothing.
MdlModel :: struct {
    status:MdlStatus;program:TiProgram;shapes:[32]TvShape;workspace:int;
    modules:[16]MdlModule;module_count:int=1;
    entries:[32]MdlEntry;count:int;top:int;
    training:bool=true;node:int=-1;child:int;
}
```

Coax passes values, not references, so every constructor takes the model and
returns a new one. The node a constructor made last comes back in `node`,
and the module it made in `child`. The first refusal is sticky: once
`status` is not `ok`, every later constructor returns the model unchanged,
so you can build a whole model and check `status` once at the end.

A name is a path of segments joined by dots. A segment is lower-case
letters, digits and underscores, so a segment can never contain a dot and
two different paths can never produce the same name. A name that is already
used by an entry or a module is refused, which is what keeps `fc1` (a
module) and `fc1` (a parameter) apart.

<!-- source: psionic/lib/module.coax names -->
```jai
// A name segment is lower-case letters, digits and underscores. Full names
// join the module path and the segment with a dot; one that would not fit
// in 63 bytes comes back empty.
mdl_segment :: (text:string)->bool {
    if text.count<1 {return false;}
    for c:text {if !((c>=97 && c<=122) || (c>=48 && c<=57) || c==95) {return false;}}
    return true;
}
mdl_join :: (prefix:MdlName,text:string)->MdlName {
    dot:=0;if prefix.count>0 {dot=1;}
    if prefix.count+dot+text.count>=MDL_NAME {return MdlName.{};}
    n:=prefix;
    if dot==1 {n.bytes[n.count]=46;n.count+=1;}
    for c:text {n.bytes[n.count]=c;n.count+=1;}
    return n;
}
mdl_name_is :: (n:MdlName,text:string)->bool {
    if n.count!=text.count {return false;}
    for i:0..<n.count {if n.bytes[i]!=text[i] {return false;}}
    return true;
}
mdl_same :: (a:MdlName,b:MdlName)->bool {
    if a.count!=b.count {return false;}
    for i:0..<a.count {if a.bytes[i]!=b.bytes[i] {return false;}}
    return true;
}
// A name is taken when an entry or a module already has it.
mdl_taken :: (m:MdlModel,n:MdlName)->bool {
    for i:0..<m.count {if mdl_same(m.entries[i].name,n) {return true;}}
    for i:1..<m.module_count {if mdl_same(m.modules[i].name,n) {return true;}}
    return false;
}
```

## Building a model

`mdl_child` adds a module, `mdl_parameter`, `mdl_buffer`, `mdl_transient`
and `mdl_data` add entries, and `mdl_node` and `mdl_op` add operations.
Every operation is checked by compiling the program so far with chapter 38's
planner, so a shape error is refused where it is made, with
`MdlStatus.program`. The compiled plan also gives each node's shape, which
the layers read, and the workspace that a forward run needs with buffer
reuse.

<!-- source: psionic/lib/module.coax build -->
```jai
mdl_model :: ()->MdlModel {return MdlModel.{};}
// A child module under parent; its id comes back in child.
mdl_child :: (m:MdlModel,parent:int,name:string)->MdlModel {
    if m.status!=MdlStatus.ok {return m;}
    if parent<0 || parent>=m.module_count {return mdl_fail(m,MdlStatus.state);}
    if !mdl_segment(name) {return mdl_fail(m,MdlStatus.name);}
    if m.module_count>=MDL_MODULES {return mdl_fail(m,MdlStatus.capacity);}
    full:=mdl_join(m.modules[parent].name,name);
    if full.count==0 || mdl_taken(m,full) {return mdl_fail(m,MdlStatus.name);}
    r:=m;r.modules[r.module_count]=MdlModule.{name=full,parent=parent};r.child=r.module_count;r.module_count+=1;
    return r;
}
// A named entry: one input node of the program and its place in the input
// buffer, right after the previous entry.
mdl_entry :: (m:MdlModel,owner:int,name:string,shape:TvShape,kind:MdlKind,init:MdlInit,value:f32)->MdlModel {
    if m.status!=MdlStatus.ok {return m;}
    if owner<0 || owner>=m.module_count {return mdl_fail(m,MdlStatus.state);}
    if !mdl_segment(name) {return mdl_fail(m,MdlStatus.name);}
    size:=tv_elements(shape);
    if size.status!=TvStatus.ok || size.elements<1 {return mdl_fail(m,MdlStatus.shape);}
    if !tv_finite(value) || (init==MdlInit.uniform && value<0) {return mdl_fail(m,MdlStatus.number);}
    if m.program.count>=TI_NODES {return mdl_fail(m,MdlStatus.capacity);}
    full:=mdl_join(m.modules[owner].name,name);
    if full.count==0 || mdl_taken(m,full) {return mdl_fail(m,MdlStatus.name);}
    r:=m;node:=r.program.count;
    r.program.nodes[node]=ti_input(shape,r.top);r.program.count+=1;r.shapes[node]=shape;
    r.entries[r.count]=MdlEntry.{name=full,kind=kind,owner=owner,node=node,shape=shape,base=r.top,size=size.elements,init=init,value=value};
    r.count+=1;r.top+=size.elements;r.node=node;
    return r;
}
mdl_parameter :: (m:MdlModel,owner:int,name:string,shape:TvShape,init:MdlInit,value:f32)->MdlModel {
    return mdl_entry(m,owner,name,shape,MdlKind.parameter,init,value);
}
mdl_buffer :: (m:MdlModel,owner:int,name:string,shape:TvShape,init:MdlInit,value:f32)->MdlModel {
    return mdl_entry(m,owner,name,shape,MdlKind.buffer,init,value);
}
mdl_transient :: (m:MdlModel,owner:int,name:string,shape:TvShape,value:f32)->MdlModel {
    return mdl_entry(m,owner,name,shape,MdlKind.transient,MdlInit.constant,value);
}
mdl_data :: (m:MdlModel,name:string,shape:TvShape)->MdlModel {
    return mdl_entry(m,MDL_ROOT,name,shape,MdlKind.data,MdlInit.constant,0);
}
// Any chapter 38 node. The program is compiled with it as the output, which
// it stays until a later node or mdl_output; workspace is that plan's.
mdl_node :: (m:MdlModel,n:TiNode)->MdlModel {
    if m.status!=MdlStatus.ok {return m;}
    if m.program.count>=TI_NODES {return mdl_fail(m,MdlStatus.capacity);}
    r:=m;node:=r.program.count;r.program.nodes[node]=n;r.program.count+=1;r.program.output=node;
    plan:=ti_compile(r.program,TiTarget.{},true);
    if plan.status!=TiStatus.ok {return mdl_fail(m,MdlStatus.program);}
    r.shapes[node]=plan.values[node].view.shape;r.workspace=plan.workspace;r.node=node;
    return r;
}
// A built-in node: a binary operator on a and b, or a sum over axis b.
mdl_op :: (m:MdlModel,op:TiOp,a:int,b:int)->MdlModel {
    n:=TiNode.{op=op,a=a};if op==TiOp.sum {n.axis=b;} else {n.b=b;}
    return mdl_node(m,n);
}
mdl_relu :: (m:MdlModel,x:int)->MdlModel {return mdl_node(m,TiNode.{op=TiOp.relu,a=x});}
mdl_output :: (m:MdlModel,x:int)->MdlModel {
    if m.status!=MdlStatus.ok {return m;}
    if !mdl_valid(m,x) || m.program.nodes[x].op==TiOp.input {return mdl_fail(m,MdlStatus.program);}
    r:=m;r.program.output=x;r.workspace=ti_compile(r.program,TiTarget.{},true).workspace;
    return r;
}
```

The layers are ordinary functions over those calls. A linear layer is a
child module with a `weight` and a `bias`, then a matrix product and an
add. Layer norm uses chapter 38's normalization, then a gain and a bias.
Dropout multiplies by a transient mask. The squared-error loss subtracts
the target (by multiplying it by a transient `-1`), squares, and sums every
axis away.

<!-- source: psionic/lib/module.coax layers -->
```jai
// Linear: y = x weight + bias for x of shape [rows, in]; weight [in, out]
// and bias [out] both drawn from [-1/sqrt(in), 1/sqrt(in)).
mdl_linear :: (m:MdlModel,parent:int,name:string,x:int,out:int)->MdlModel {
    if m.status!=MdlStatus.ok {return m;}
    if !mdl_valid(m,x) {return mdl_fail(m,MdlStatus.program);}
    s:=m.shapes[x];
    if s.rank!=2 || out<1 {return mdl_fail(m,MdlStatus.shape);}
    bound:=f32(1/sqrt(f64(s.dims[1])));
    r:=mdl_child(m,parent,name);me:=r.child;
    r=mdl_parameter(r,me,"weight",mdl_shape(2,s.dims[1],out),MdlInit.uniform,bound);w:=r.node;
    r=mdl_parameter(r,me,"bias",mdl_shape(1,out,0),MdlInit.uniform,bound);b:=r.node;
    r=mdl_op(r,TiOp.matmul,x,w);
    return mdl_op(r,TiOp.add,r.node,b);
}
// Layer norm over the last axis, then a gain (ones) and a bias (zeros).
mdl_layernorm :: (m:MdlModel,parent:int,name:string,x:int)->MdlModel {
    if m.status!=MdlStatus.ok {return m;}
    if !mdl_valid(m,x) {return mdl_fail(m,MdlStatus.program);}
    s:=m.shapes[x];
    if s.rank<1 {return mdl_fail(m,MdlStatus.shape);}
    width:=mdl_shape(1,s.dims[s.rank-1],0);
    r:=mdl_child(m,parent,name);me:=r.child;
    r=mdl_node(r,TiNode.{op=TiOp.layernorm,a=x});h:=r.node;
    r=mdl_parameter(r,me,"weight",width,MdlInit.constant,1);r=mdl_op(r,TiOp.multiply,h,r.node);h=r.node;
    r=mdl_parameter(r,me,"bias",width,MdlInit.constant,0);
    return mdl_op(r,TiOp.add,h,r.node);
}
// Dropout of drop/256 of the elements, 0 <= drop < 256: x times a
// transient mask that mdl_refresh rebuilds before each run.
mdl_dropout :: (m:MdlModel,parent:int,name:string,x:int,drop:int)->MdlModel {
    if m.status!=MdlStatus.ok {return m;}
    if !mdl_valid(m,x) {return mdl_fail(m,MdlStatus.program);}
    if drop<0 || drop>255 {return mdl_fail(m,MdlStatus.shape);}
    r:=mdl_child(m,parent,name);
    r=mdl_transient(r,r.child,"mask",m.shapes[x],1);
    r.entries[r.count-1].mask=true;r.entries[r.count-1].drop=drop;
    return mdl_op(r,TiOp.multiply,x,r.node);
}
// The sum of squared differences between y and target.
mdl_squared_error :: (m:MdlModel,parent:int,name:string,y:int,target:int)->MdlModel {
    r:=mdl_child(m,parent,name);
    r=mdl_transient(r,r.child,"neg",mdl_shape(1,1,0),-1);
    r=mdl_op(r,TiOp.multiply,target,r.node);r=mdl_op(r,TiOp.add,y,r.node);
    r=mdl_op(r,TiOp.multiply,r.node,r.node);
    while r.status==MdlStatus.ok && r.shapes[r.node].rank>0 {r=mdl_op(r,TiOp.sum,r.node,0);}
    return r;
}
```

The example builds a pre-norm residual MLP block, the feed-forward half of
a transformer decoder block, with a squared-error loss on top:
`y = x + scale * fc2(drop(relu(fc1(norm(x)))))`.

<!-- source: psionic/examples/module-block.coax build -->
```jai
// A pre-norm residual MLP block, the feed-forward half of a transformer
// decoder block, with a squared-error loss:
// y = x + scale * fc2(drop(relu(fc1(norm(x))))).
mb_block :: (width:int,hidden:int,drop:int)->MdlModel {
    m:=mdl_model();
    m=mdl_data(m,"x",mdl_shape(2,4,width));x:=m.node;
    m=mdl_data(m,"target",mdl_shape(2,4,width));target:=m.node;
    m=mdl_layernorm(m,MDL_ROOT,"norm",x);
    m=mdl_linear(m,MDL_ROOT,"fc1",m.node,hidden);
    m=mdl_relu(m,m.node);
    m=mdl_dropout(m,MDL_ROOT,"drop",m.node,drop);
    m=mdl_linear(m,MDL_ROOT,"fc2",m.node,width);h:=m.node;
    m=mdl_buffer(m,MDL_ROOT,"scale",mdl_shape(1,width,0),MdlInit.constant,0.5);
    m=mdl_op(m,TiOp.multiply,h,m.node);
    m=mdl_op(m,TiOp.add,m.node,x);
    return mdl_squared_error(m,MDL_ROOT,"loss",m.node,target);
}
```

## Working the layout by hand

Before running anything, you can predict the first part of the output from
the rules above. Entries are placed in the order they are registered, each
right after the previous one. For width 4, hidden width 8 and a batch of 4
rows:

| Entry | Kind | Shape | Size | Base |
| --- | --- | --- | --- | --- |
| `x` | data | [4,4] | 16 | 0 |
| `target` | data | [4,4] | 16 | 16 |
| `norm.weight` | parameter | [4] | 4 | 32 |
| `norm.bias` | parameter | [4] | 4 | 36 |
| `fc1.weight` | parameter | [4,8] | 32 | 40 |
| `fc1.bias` | parameter | [8] | 8 | 72 |
| `drop.mask` | transient | [4,8] | 32 | 80 |
| `fc2.weight` | parameter | [8,4] | 32 | 112 |
| `fc2.bias` | parameter | [4] | 4 | 144 |
| `scale` | buffer | [4] | 4 | 148 |
| `loss.neg` | transient | [1] | 1 | 152 |

The input buffer needs 152 + 1 = 153 floats. The trainable floats are the
parameters: 4 + 4 + 32 + 8 + 32 + 4 = 84. The saved floats add the buffer:
84 + 4 = 88. `fc1` reads 4 features, so its values are drawn from
[-1/sqrt(4), 1/sqrt(4)) = [-0.5, 0.5); `fc2` reads 8, so its bound is
1/sqrt(8) = 0.353553. The modules are the root, `norm`, `fc1`, `drop`, `fc2`
and `loss`: six. The program has 11 input nodes and 16 operations: layer
norm, its gain and bias (3), two linear layers (2 each), ReLU, dropout, the
scale and the residual add (2), and the loss (multiply, add, square and two
sums: 5), which is 27 nodes.

The state dictionary is 7 tensors and 88 × 4 = 352 bytes of values. A
chapter 8 file is an 8-byte header length, the header, and the values, so
the file is 8 + header + 352 bytes. The header here is 512 bytes of JSON
(padded to a multiple of 8), so the file is 872 bytes.

Dropout drops `drop`/256 of the elements. At 128 that is half, and the kept
elements are scaled by 256 / (256 - 128) = 2, so the mask is 0 or 2. In
eval mode the mask is all ones, and x × 1 = x exactly in f32, so dropout in
eval mode is the identity bit for bit.

## Names, modes and freezing

`mdl_find` looks an entry up by name. `mdl_train` switches between train
and eval mode; only `mdl_refresh`, which rebuilds transients before a run,
reads it. `mdl_freeze` freezes every parameter under a name, so
`mdl_freeze(m, "fc1", true)` freezes `fc1.weight` and `fc1.bias`, while
`"fc"` matches nothing and is refused. `mdl_mode` turns the model into
chapter 106's `AdMode`: a gradient for every parameter that is not frozen.
A frozen parameter is not asked for a gradient at all, so the tape does no
backward work for it.

<!-- source: psionic/lib/module.coax state -->
```jai
mdl_find :: (m:MdlModel,name:string)->int {
    if m.status!=MdlStatus.ok {return -1;}
    for i:0..<m.count {if mdl_name_is(m.entries[i].name,name) {return i;}}
    return -1;
}
mdl_saved :: (e:MdlEntry)->bool {return e.kind==MdlKind.parameter || e.kind==MdlKind.buffer;}
mdl_parameters :: (m:MdlModel)->int {n:=0;for i:0..<m.count {if m.entries[i].kind==MdlKind.parameter {n+=m.entries[i].size;}}return n;}
mdl_state_floats :: (m:MdlModel)->int {n:=0;for i:0..<m.count {if mdl_saved(m.entries[i]) {n+=m.entries[i].size;}}return n;}
// Train or eval mode. Only mdl_refresh reads it.
mdl_train :: (m:MdlModel,training:bool)->MdlModel {
    if m.status!=MdlStatus.ok {return m;}
    r:=m;r.training=training;return r;
}
// Freeze or unfreeze every parameter whose name is prefix or starts with
// prefix and a dot; "" names them all. Naming no parameter is refused.
mdl_freeze :: (m:MdlModel,prefix:string,frozen:bool)->MdlModel {
    if m.status!=MdlStatus.ok {return m;}
    r:=m;hit:=false;
    for i:0..<r.count {
        e:=r.entries[i];if e.kind!=MdlKind.parameter || e.name.count<prefix.count {continue;}
        match:=prefix.count==0 || e.name.count==prefix.count || e.name.bytes[prefix.count]==46;
        for j:0..<prefix.count {if e.name.bytes[j]!=prefix[j] {match=false;}}
        if match {r.entries[i].frozen=frozen;hit=true;}
    }
    if !hit {return mdl_fail(m,MdlStatus.name);}
    return r;
}
// Chapter 106's mode: a gradient for every parameter that is not frozen.
mdl_mode :: (m:MdlModel)->AdMode {
    mode:AdMode;if m.status!=MdlStatus.ok {return mode;}
    for i:0..<m.count {e:=m.entries[i];if e.kind==MdlKind.parameter && !e.frozen {mode.grad[e.node]=true;}}
    return mode;
}
```

`mdl_init` fills parameters and buffers by their rules, `mdl_refresh`
rebuilds transients, and `mdl_sgd` takes one plain gradient-descent step.
Each element's random draw is keyed by the seed and its offset in the input
buffer, so the same model and seed always give the same values. A dropout
mask element is kept when the top byte of the counter generator's word for
(step, offset) is at least `drop`; using a byte keeps the rule exact, with
no comparison between a random double and a rounded probability.

<!-- source: psionic/lib/module.coax fill -->
```jai
// Fill parameters and buffers by their rules. Element j of an entry at
// base b draws counter b + j, so the values depend only on the seed and the
// layout.
mdl_init :: (m:MdlModel,seed:u64,inputs:[]f32)->MdlStatus {
    if m.status!=MdlStatus.ok {return MdlStatus.state;}
    if inputs.count<m.top {return MdlStatus.capacity;}
    for i:0..<m.count {
        e:=m.entries[i];if !mdl_saved(e) {continue;}
        for j:0..<e.size {
            v:=f64(e.value);
            if e.init==MdlInit.uniform {v*=2*cr_draw(CrSeed.{seed=seed,counter=cast(u64)(e.base+j)}).value-1;}
            inputs[e.base+j]=f32(v);
        }
    }
    return MdlStatus.ok;
}
// Rebuild transient entries for one run. In train mode a dropout mask keeps
// element j when the top byte of the counter generator's word for
// (step, b + j) is at least drop, and scales it by 256 / (256 - drop). In
// eval mode a mask is ones. Other transients take their value.
mdl_refresh :: (m:MdlModel,step:u64,inputs:[]f32)->MdlStatus {
    if m.status!=MdlStatus.ok {return MdlStatus.state;}
    if inputs.count<m.top {return MdlStatus.capacity;}
    for i:0..<m.count {
        e:=m.entries[i];if e.kind!=MdlKind.transient {continue;}
        keep:=f32(256/f64(256-e.drop));
        for j:0..<e.size {
            v:=e.value;
            if m.training && e.mask {
                v=0;if cast(int)(cr_bits(step,cast(u64)(e.base+j))>>56)>=e.drop {v=keep;}
            }
            inputs[e.base+j]=v;
        }
    }
    return MdlStatus.ok;
}
// One plain gradient-descent step on every trainable parameter, after the
// tape t (built from this model's program and mdl_mode) has run to the end.
// g is scratch for every trainable float; all gradients are published
// before any parameter changes.
mdl_sgd :: (m:MdlModel,t:AdTape,cursor:AdCursor,grads:[]f64,g:[]f32,inputs:[]f32,rate:f32)->MdlStatus {
    if m.status!=MdlStatus.ok || t.status!=AdStatus.ok || cursor.next<t.count {return MdlStatus.state;}
    if !(rate>0) || !tv_finite(rate) {return MdlStatus.number;}
    if inputs.count<m.top || g.count<mdl_parameters(m) {return MdlStatus.capacity;}
    mode:=mdl_mode(m);for i:0..<TI_NODES {if mode.grad[i]!=t.mode.grad[i] {return MdlStatus.state;}}
    at:=0;
    for i:0..<m.count {
        e:=m.entries[i];if !mode.grad[e.node] {continue;}
        if ad_gradient(t,cursor,e.node,grads,g[at..<at+e.size])!=AdStatus.ok {return MdlStatus.number;}
        at+=e.size;
    }
    at=0;
    for i:0..<m.count {
        e:=m.entries[i];if !mode.grad[e.node] {continue;}
        for j:0..<e.size {inputs[e.base+j]=f32(f64(inputs[e.base+j])-f64(rate)*f64(g[at+j]));}
        at+=e.size;
    }
    return MdlStatus.ok;
}
```

`mdl_sgd` refuses a tape whose gradient flags differ from the model's
current freezing, so a tape built before a parameter was frozen cannot
update it. Real optimizers are [chapter 124](ch124-adamw-and-explicit-precision.md)'s;
`mdl_sgd` exists so the freezing check has something to train with.

A parameter is shared by using its node twice. It has one entry, one name
and one place in the state dictionary. Chapter 106's tape sums the
gradients from every use; a check compares a parameter used twice with two
separate copies and finds that its gradient is the sum of theirs, exactly.

## Saving and loading by name

`mdl_save` writes the parameters and buffers, in registration order, as a
chapter 8 file of little-endian f32 values. Chapter 8 sorts tensors by
name in the file, so the file does not depend on the order a model was
built in.

<!-- source: psionic/lib/module.coax save -->
```jai
// The state dictionary: parameters and buffers, by name, as chapter 8's
// file of little-endian f32 values. out doubles as the staging area for the
// values, so after a refusal for a non-finite value its contents are
// unspecified.
mdl_save :: (m:MdlModel,inputs:[]f32,out:[]u8,stage:[]u8)->MdlSaved {
    if m.status!=MdlStatus.ok {return MdlSaved.{status=MdlStatus.state};}
    if inputs.count<m.top {return MdlSaved.{status=MdlStatus.capacity};}
    tensors:[32]IcTensor;n:=0;
    for i:0..<m.count {
        e:=m.entries[i];if !mdl_saved(e) {continue;}
        t:IcTensor;t.name_count=e.name.count;for j:0..<e.name.count {t.name[j]=e.name.bytes[j];}
        t.shape=e.shape;t.bytes=4*e.size;tensors[n]=t;n+=1;
    }
    p:=ic_plan(tensors[0..<n]);
    if p.status!=IcStatus.ok || out.count<p.total || stage.count<p.total {return MdlSaved.{status=MdlStatus.capacity};}
    at:=0;
    for i:0..<m.count {
        e:=m.entries[i];if !mdl_saved(e) {continue;}
        for j:0..<e.size {w:=f32_bits(inputs[e.base+j]);for k:0..<4 {out[at+k]=cast,trunc(u8)(w>>cast(u32)(8*k));}at+=4;}
    }
    r:=ic_write(tensors[0..<n],out[0..<p.payload],out,stage,false);
    if r.status!=IcStatus.ok {return MdlSaved.{status=MdlStatus.number};}
    return MdlSaved.{written=r.written,tensors=n};
}
```

`mdl_load` matches file tensors to entries by name. It checks everything
before it writes a single value: a tensor with the right name but a
different shape or dtype is refused with `mismatch`, and with `all_keys`
an entry the file lacks is refused with `missing` and a file tensor the
model lacks with `unexpected`. Without `all_keys`, missing and unexpected
names are counted and every matching tensor loads, which is how you load a
part of a model into a larger one.

<!-- source: psionic/lib/module.coax load -->
```jai
mdl_named :: (f:IcTensor,n:MdlName)->bool {
    if f.name_count!=n.count {return false;}
    for j:0..<n.count {if f.name[j]!=n.bytes[j] {return false;}}
    return true;
}
mdl_fits :: (f:IcTensor,e:MdlEntry)->bool {
    if f.dtype!=0 || f.shape.rank!=e.shape.rank {return false;}
    for d:0..<e.shape.rank {if f.shape.dims[d]!=e.shape.dims[d] {return false;}}
    return true;
}
// Load a state dictionary by name. Every check runs before any value is
// written. With all_keys, the first missing entry or else the first
// unexpected file tensor is refused; without it they are counted and the
// rest load. A shape or dtype that differs is always refused.
mdl_load :: (m:MdlModel,file:[]u8,inputs:[]f32,all_keys:bool)->MdlLoad {
    if m.status!=MdlStatus.ok {return MdlLoad.{status=MdlStatus.state};}
    if inputs.count<m.top {return MdlLoad.{status=MdlStatus.capacity};}
    found:[16]IcTensor;r:=ic_inspect(file,found[..],false);
    if r.status!=IcStatus.ok {return MdlLoad.{status=MdlStatus.format};}
    l:MdlLoad;match:[32]int;used:[16]bool;
    for i:0..<m.count {
        match[i]=-1;e:=m.entries[i];if !mdl_saved(e) {continue;}
        for k:0..<r.written {if mdl_named(found[k],e.name) {match[i]=k;used[k]=true;}}
        if match[i]<0 {l.missing+=1;continue;}
        if !mdl_fits(found[match[i]],e) {return MdlLoad.{status=MdlStatus.mismatch,entry=i,file=match[i]};}
        l.loaded+=1;
    }
    for k:0..<r.written {if !used[k] {l.unexpected+=1;}}
    if all_keys {
        for i:0..<m.count {if mdl_saved(m.entries[i]) && match[i]<0 {return MdlLoad.{status=MdlStatus.missing,entry=i,missing=l.missing,unexpected=l.unexpected};}}
        for k:0..<r.written {if !used[k] {return MdlLoad.{status=MdlStatus.unexpected,file=k,loaded=l.loaded,unexpected=l.unexpected};}}
    }
    for i:0..<m.count {
        k:=match[i];if k<0 {continue;}
        e:=m.entries[i];
        for j:0..<e.size {
            w:u32=0;for b:0..<4 {w|=cast(u32)file[found[k].source+4*j+b]<<cast(u32)(8*b);}
            inputs[e.base+j]=f32_from_bits(w);
        }
    }
    return l;
}
```

## The program

<!-- source: psionic/examples/module-block.coax example -->
```jai
main :: () {
    m:=mb_block(4,8,128);buf:[64]u8;
    print("model: status %, % modules, % entries, % program nodes, % input floats, workspace % floats\n",m.status,m.module_count,m.count,m.program.count,m.top,m.workspace);
    for i:0..<m.count {
        e:=m.entries[i];
        print("  %  % [",name_of(e.name,buf[..]),MB_KINDS[cast(int)e.kind]);
        for d:0..<e.shape.rank {if d>0 {print(",");}print("%",e.shape.dims[d]);}
        print("] at %",e.base);
        if e.kind==MdlKind.parameter || e.kind==MdlKind.buffer {print(", % %",ifx e.init==MdlInit.uniform then "uniform +-" else "constant",mb_r6(f64(e.value)));}
        print("\n");
    }
    print("trainable floats %, saved floats %\n",mdl_parameters(m),mdl_state_floats(m));

    // Values: parameters by their rules (seed 7), data by formula.
    inputs:[256]f32;#assert mdl_init(m,7,inputs[..])==MdlStatus.ok;
    for j:0..<16 {
        x:=f64((j*5)%16-8)/8;inputs[j]=f32(x);
        inputs[16+j]=f32(f64((((j/4)*4+(j+1)%4)*5)%16-8)/16);
    }

    // The same block wired node by node, in its own layout.
    w:=mdl_test_wired();wired:[512]f32;
    for k:0..<10 {e:=m.entries[k];for j:0..<e.size {wired[mdl_test_wired_base[k]+j]=inputs[e.base+j];}}
    wired[352]=-1;
    #assert mdl_refresh(m,1,inputs[..])==MdlStatus.ok;
    for j:0..<32 {wired[280+j]=inputs[80+j];}
    t:=ad_tape(m.program,mdl_mode(m),OrRegistry.{});
    wmode:AdMode;for k:2..<9 {if m.entries[k].kind==MdlKind.parameter {wmode.grad[mdl_test_wired_node[k]]=true;}}
    tw:=ad_tape(w,wmode,OrRegistry.{});
    values:[1024]f32;grads:[1024]f64;flags:[64]u8;wv:[1024]f32;wg:[1024]f64;
    run:=ad_run(t,AdCursor.{},inputs[..],values[..],grads[..],flags[..]);
    rw:=ad_run(tw,AdCursor.{},wired[..],wv[..],wg[..],flags[..]);
    a:[1]f32;b:[1]f32;
    #assert ad_value(t,run.cursor,26,inputs[..],values[..],a[..])==AdStatus.ok && ad_value(tw,rw.cursor,26,wired[..],wv[..],b[..])==AdStatus.ok;
    print("\ncomposed against hand-wired, training mode, mask from step 1:\n");
    print("  loss % and %, same bits: %\n",mb_r6(f64(a[0])),mb_r6(f64(b[0])),f32_bits(a[0])==f32_bits(b[0]));
    g:[64]f32;gw:[64]f32;
    for k:2..<9 {
        e:=m.entries[k];if e.kind!=MdlKind.parameter {continue;}
        #assert ad_gradient(t,run.cursor,e.node,grads[..],g[..])==AdStatus.ok && ad_gradient(tw,rw.cursor,mdl_test_wired_node[k],wg[..],gw[..])==AdStatus.ok;
        same:=true;norm:f64=0;for j:0..<e.size {if f32_bits(g[j])!=f32_bits(gw[j]) {same=false;}norm+=f64(g[j])*f64(g[j]);}
        print("  gradient of %: norm %, same bits: %\n",name_of(e.name,buf[..]),mb_r6(sqrt(norm)),same);
    }

    // Train with the norm frozen and dropout on; evaluate in eval mode.
    f:=mdl_freeze(m,"norm",true);t=ad_tape(f.program,mdl_mode(f),OrRegistry.{});
    norm_before:[8]f32;g84:[84]f32;for j:0..<8 {norm_before[j]=inputs[32+j];}
    print("\ntraining with norm frozen, dropout 128/256, learning rate 0.02:\n");
    for step:0..<401 {
        if step%100==0 {print("  step %: eval loss %\n",step,mb_r6(f64(mb_loss(f,inputs[..]))));}
        if step==400 {break;}
        #assert mdl_refresh(f,cast(u64)step+100,inputs[..])==MdlStatus.ok;
        run=ad_run(t,AdCursor.{},inputs[..],values[..],grads[..],flags[..]);
        #assert run.status==AdStatus.ok && mdl_sgd(f,t,run.cursor,grads[..],g84[..],inputs[..],0.02)==MdlStatus.ok;
    }
    unchanged:=true;for j:0..<8 {if f32_bits(inputs[32+j])!=f32_bits(norm_before[j]) {unchanged=false;}}
    print("  norm.weight and norm.bias unchanged: %\n",unchanged);

    // Save by name, load into a fresh buffer, and compare.
    file:[4096]u8;stage:[4096]u8;s:=mdl_save(f,inputs[..],file[..],stage[..]);
    print("\nstate dictionary: %, % tensors, % bytes\n",s.status,s.tensors,s.written);
    fresh:[512]f32;for j:0..<32 {fresh[j]=inputs[j];}
    l:=mdl_load(mb_block(4,8,128),file[0..<s.written],fresh[..],true);
    before:=mb_loss(f,inputs[..]);after:=mb_loss(f,fresh[..]);
    print("loaded: %, % tensors; eval loss % before and % after, same bits: %\n",l.status,l.loaded,mb_r6(f64(before)),mb_r6(f64(after)),f32_bits(before)==f32_bits(after));

    // Refusals. Nothing is written when a load is refused.
    print("\nrefused loads:\n");
    extra:=mdl_parameter(mb_block(4,8,128),MDL_ROOT,"gate",mdl_shape(1,4,0),MdlInit.constant,1);
    l=mdl_load(extra,file[0..<s.written],fresh[..],true);
    print("  model with an extra gate: %, entry %\n",l.status,name_of(extra.entries[l.entry].name,buf[..]));
    small:=mdl_model();small=mdl_data(small,"x",mdl_shape(2,4,4));
    small=mdl_layernorm(small,MDL_ROOT,"norm",0);small=mdl_linear(small,MDL_ROOT,"fc1",small.node,8);
    small=mdl_relu(small,small.node);small=mdl_linear(small,MDL_ROOT,"fc2",small.node,4);
    l=mdl_load(small,file[0..<s.written],fresh[..],true);
    found:[16]IcTensor;#assert ic_inspect(file[0..<s.written],found[..],false).status==IcStatus.ok;
    print("  model without scale: %, % unexpected, file tensor %\n",l.status,l.unexpected,cast(string)found[l.file].name[0..<found[l.file].name_count]);
    loose:=mdl_load(small,file[0..<s.written],fresh[..],false);
    print("  the same, not requiring every key: %, % loaded, % unexpected\n",loose.status,loose.loaded,loose.unexpected);
    wide:=mb_block(4,16,128);l=mdl_load(wide,file[0..<s.written],fresh[..],false);
    print("  fc1 16 wide: %, entry %\n",l.status,name_of(wide.entries[l.entry].name,buf[..]));
    file[s.written-1]^=1;file[20]=0;
    l=mdl_load(f,file[0..<s.written],fresh[..],true);print("  damaged header: %\n",l.status);
    print("  bad names: % % %\n",mdl_data(m,"fc1",mdl_shape(1,1,0)).status,mdl_data(m,"a.b",mdl_shape(1,1,0)).status,mdl_freeze(m,"fc",true).status);
}
```

The output shows each part of the chapter:

- The layout matches the table worked out by hand: 153 floats, 84
  trainable and 88 saved, bounds 0.5 and 0.353553, 872 bytes on disk.
- The composed block and the hand-wired program from `mdl_test_wired` give
  the same loss and the same six gradients, bit for bit.
- With `norm` frozen, 400 steps of training with dropout lower the eval
  loss from 9.12 to about 0.90, and `norm.weight` and `norm.bias` keep
  their exact bits.
- The saved state loads into a fresh buffer and gives the same eval loss,
  bit for bit.
- Each deliberately broken load is refused with its own status, and names
  the entry or file tensor at fault: an extra parameter (`missing`, entry
  `gate`), a model without `scale` (`unexpected`, file tensor `scale`), a
  16-wide `fc1` (`mismatch`, entry `fc1.weight`), and a damaged header
  (`format`). Bad names are refused with `name`.

## How it is checked

[`module.coax`](../lib/module.coax) holds the checks, written before the code:

- **The composed block against the hand-wired program.** The two programs
  are compared node by node (operation, operands, axis, input shapes), then
  run with the same values: the loss and every parameter gradient agree bit
  for bit, and every gradient has a nonzero element.
- **Layout and names:** every entry's name, kind, base and size against the
  table above; the module tree; the counts; node shapes; the workspace
  against chapter 38's planner run directly.
- **Initialization:** constants exactly, uniform values against the
  counter generator directly, the same seed giving the same bits and
  another seed different ones, data and transients untouched.
- **Dropout:** for every `drop` from 0 to 255, every mask element against
  the byte rule computed directly; eval mode as ones; eval-mode dropout
  equal to no dropout bit for bit, and train mode not.
- **Freezing:** three training steps lower the loss, frozen parameters keep
  their bits, trainable ones move, and buffers stay fixed. On a model whose
  first entry is a parameter, one step is `w - rate * x` exactly.
- **Sharing:** a parameter used twice gets the exact sum of the gradients
  of two separate copies.
- **State dictionaries:** a round trip restores every saved float bit for
  bit and leaves transients and data alone; a model built in another order
  loads by name; missing, unexpected, near-miss, mismatched-shape,
  mismatched-rank and BF16 tensors are refused with nothing written.
- **Construction:** names at the 63-byte limit and one past it, character
  classes, duplicate names, module and node capacity, operand and shape
  refusals for every layer, and the sticky refusal.
- **Mutation:** `coax mutate psionic/lib/module.coax --root-only` kills
  every mutant.

## What is not here yet

- **Convolutions and attention.** Chapter 38's IR has matrix products,
  elementwise sums and products, ReLU, sums, layer norm and gathers. A
  convolution, softmax or exponential is not one of them, so a CNN or the
  attention half of a transformer block cannot be built from this API yet.
  When the IR gains those operations, they become layers here the same way
  `mdl_linear` is.
- **Larger models.** One program holds 32 nodes, which is one block like the
  example's. Stacking blocks needs chapter 38's limit raised or programs
  chained.
- **Registered operators as layers.** `mdl_node` accepts any node, but the
  layers here use built-in operations only; a layer around a
  [chapter 40](ch040-add-your-own-operator.md) operator would carry its
  registry with the model.
- **A trainer.** `mdl_sgd` is plain gradient descent for the checks. The
  trainer, its optimizers and its checkpoints are chapters
  [118](ch118-one-trainer-for-every-model.md),
  [124](ch124-adamw-and-explicit-precision.md) and
  [122](ch122-a-checkpoint-holds-the-whole-experiment.md).

## Public API

| Call | What it does |
| --- | --- |
| `mdl_model()` | An empty model with its root module, `MDL_ROOT`. |
| `mdl_child(m, parent, name)` | Adds a module; its id is in `child`. |
| `mdl_parameter`, `mdl_buffer` `(m, owner, name, shape, init, value)` | Adds a trained, or a saved but untrained, tensor. |
| `mdl_transient(m, owner, name, shape, value)`, `mdl_data(m, name, shape)` | Adds a tensor rebuilt each run, or a batch input. |
| `mdl_node(m, node)`, `mdl_op(m, op, a, b)`, `mdl_relu(m, x)` | Adds an operation; its node is in `node`. |
| `mdl_linear`, `mdl_layernorm`, `mdl_dropout`, `mdl_squared_error` | The layers. |
| `mdl_output(m, x)` | Chooses the output node (by default, the last). |
| `mdl_find(m, name)` | An entry's index, or -1. |
| `mdl_parameters(m)`, `mdl_state_floats(m)` | Trainable and saved float counts. |
| `mdl_train(m, training)`, `mdl_freeze(m, prefix, frozen)`, `mdl_mode(m)` | Modes, freezing, and chapter 106's gradient flags. |
| `mdl_init(m, seed, inputs)`, `mdl_refresh(m, step, inputs)` | Fills parameters and buffers; rebuilds transients. |
| `mdl_sgd(m, tape, cursor, grads, scratch, inputs, rate)` | One gradient-descent step. |
| `mdl_save(m, inputs, out, stage)`, `mdl_load(m, file, inputs, all_keys)` | The state dictionary by name. |
