# Restore the retained model and infer with public APIs

Start at the repository root. Read the retained
[`run-v3/model.safetensors`](run-v3/model.safetensors) as an immutable byte
buffer in your platform host. It contains eight F32 tensors; it is the
inference state, rather than the optimizer checkpoint. The public
[`mdl_load`](../../lib/module.coax) takes those bytes, not a filename. There
is no generic `mdl_load_path` or artifact-inference CLI in this profile;
ordinary host file I/O supplies the bytes. No experiment harness is required.

Save a new `reader-residual.coax` at the repository root. Load the public
libraries below, then use the exact constructor region copied from the
checked model source. `rml_config` is included to keep the source mapping
exact; inference does not call it or run an optimizer.

```jai
#load "psionic/lib/module-residual.coax";
#load "psionic/lib/trainer.coax";
```

<!-- source: psionic/lib/residual-mlp-classifier.coax model -->
```jai
RmlModel :: struct {model:MdlModel;prediction:int;}
rml_definition :: ()->RmlModel {
    m:=mdl_model();m=mdl_data(m,"x",TvShape.{rank=4,dims=[4]int.[1,1,14,14]});x:=m.node;
    m=mdl_data(m,"target",mdl_shape(2,1,10));target:=m.node;
    m=mdl_residual_conv1x1(m,MDL_ROOT,"block",x,2);
    m=mdl_flatten(m,m.node);m=mdl_linear(m,MDL_ROOT,"fc1",m.node,16);m=mdl_relu(m,m.node);
    m=mdl_linear(m,MDL_ROOT,"fc2",m.node,10);prediction:=m.node;
    m=mdl_squared_error(m,MDL_ROOT,"loss",prediction,target);
    return RmlModel.{model=m,prediction=prediction};
}
rml_model :: ()->MdlModel {return rml_definition().model;}
rml_config :: ()->TrConfig {
    return TrConfig.{optimizer=TrOptimizer.adamw,storage=TrStorage.binary64,
        rate=0.003,beta1=0.9,beta2=0.999,eps=0.00000001,clip=1,bound=16,
        examples=4096,batch=1,accumulate=16,epochs=12,shuffle=true,seed=11822};
}
```

The constructor registers these names and shapes. `mdl_load(..., true)`
binds them by name to the registered input-buffer locations and refuses
missing/extra keys or a shape/dtype mismatch. Do not copy payload offsets or
assume the file's alphabetical order equals registration order.

| F32 tensor | Shape | Values |
| --- | --- | --- |
| `block.expand.weight` | `[2,1,1,1]` | 2 |
| `block.expand.bias` | `[1,2,1,1]` | 2 |
| `block.contract.weight` | `[1,2,1,1]` | 2 |
| `block.contract.bias` | `[1,1,1,1]` | 1 |
| `fc1.weight` | `[196,16]` | 3,136 |
| `fc1.bias` | `[16]` | 16 |
| `fc2.weight` | `[16,10]` | 160 |
| `fc2.bias` | `[10]` | 10 |

Append this public-API inference entry. Its fixed arenas are this bounded
model's existing caller capacities. The host supplies 196 pooled f32 features
and storage for ten output scores; all spans stay live and disjoint. The
load replaces every saved parameter, so the temporary initialization seed
has no effect on the restored state. `prediction` is retained by the
constructor; callers do not guess an internal node number.

```jai
residual_saved_logits :: (file:[]u8,features:[]f32,scores:[]f32)->bool #export_c {
    if features.count!=196 || scores.count!=10 {return false;}
    def:=rml_definition();m:=mdl_train(def.model,false);
    inputs:[3536]f32;
    if mdl_init(m,0,inputs[..])!=MdlStatus.ok {return false;}
    loaded:=mdl_load(m,file,inputs[..],true);
    if loaded.status!=MdlStatus.ok || loaded.loaded!=8 {return false;}
    for j:0..<196 {inputs[j]=features[j];}
    if mdl_refresh(m,0,inputs[..])!=MdlStatus.ok {return false;}
    f:=mdl_output(m,def.prediction);
    tape:=ad_tape(f.program,AdMode.{enabled=false},OrRegistry.{});
    if tape.status!=AdStatus.ok {return false;}
    values:[16384]f32;adjoints:[16384]f64;flags:[4096]u8;staged:[10]f32;
    run:=ad_run(tape,AdCursor.{},inputs[..],values[..],adjoints[..],flags[..]);
    if run.status!=AdStatus.ok {return false;}
    if ad_value(tape,run.cursor,def.prediction,inputs[..],values[..],staged[..])!=AdStatus.ok {return false;}
    for j:0..<10 {scores[j]=staged[j];}
    return true;
}
```

Emit its C/header with the selected qualified compiler:

```sh
"$COAX" c reader-residual.coax --library --header /tmp/reader-residual.h > /tmp/reader-residual.c
```

Your C host includes `reader-residual.h`, reads the state with ordinary
`fopen`/`fread` (checking the real byte count), and calls the generated
`residual_saved_logits` with the generated public ABI views:

```text
int ok = residual_saved_logits((coax_v_u8){state_bytes,state_count},
    (coax_v_f32){features,196}, (coax_v_f32){scores,10});
```

These are generated public ABI types, not private experiment functions. Link the generated C and your host with
`cc -std=c11 -O2 -ffp-contract=off -I/tmp /tmp/reader-residual.c YOUR_HOST.c -lm`.
File access is the host's only extra responsibility; constructor, state
validation, arithmetic and inference remain Coax.

For original MNIST input, the public
[`data/mnist.coax`](../../lib/data/mnist.coax) provides
`mn_admit(image_bytes,label_bytes)` and `mn_pool(admitted,image_bytes,row,features)`.
Add `#load "psionic/lib/data/mnist.coax";` to your new source, then an
optional entry that accepts those two IDX files as bytes:

```jai
residual_saved_row0 :: (state:[]u8,images:[]u8,labels:[]u8,scores:[]f32)->bool #export_c {
    d:=mn_admit(images,labels);if d.status!=MnStatus.ok {return false;}
    features:[196]f32;
    if mn_pool(d,images,0,features[..])!=MnStatus.ok {return false;}
    return residual_saved_logits(state,features[..],scores);
}
```

Use the authenticated cache's `t10k-images-idx3-ubyte` and
`t10k-labels-idx1-ubyte`; pooling row 0 produces the 196 NCHW features for
`[1,1,14,14]`. Its restored ten scores correspond to the first ten F32 words
of [`run-v3/test-scores.f32`](run-v3/test-scores.f32). The cache stays outside
the repository. Reading raw 28×28 pixels directly as 196 features is not this
model's preprocessing.

Existing analogous public builders and retained states are:

| Model | State file | Builder and forward-only tape |
| --- | --- | --- |
| MNIST MLP | [`../mnist/run-v1/model.safetensors`](../mnist/run-v1/model.safetensors) | [`mc_model()`, `mc_inference(m)`](../../lib/mnist-classifier.coax); four saved tensors |
| MNIST CNN | [`../cnn/run-v1/model.safetensors`](../cnn/run-v1/model.safetensors) | [`cn_model()`, `cn_inference(m)`](../../lib/cnn-classifier.coax); four saved tensors |

For those tapes, their `program.output` is the score node; their own state
shapes and input capacities apply. The general load/run/read sequence is the
same. This recipe edit adds source/link metadata only; a fresh-reader follow-up
records its own compilation and runtime observations separately.
