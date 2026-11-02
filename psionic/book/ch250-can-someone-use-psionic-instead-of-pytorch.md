# 250. Can someone use Psionic instead of PyTorch?

> **Status: partly implemented.** A model audit checks a model, described
> as its PyTorch layers, against a 32-operator table before any memory is
> allocated. A generated matrix ties every capability claim to the chapter
> registry. Nine standard workflows run end to end with recorded results, and
> an opt-in PyTorch comparator matches chapter 118's two MNIST models on 2,048
> of 2,048 test images. Still to do: a package for outside users, the three
> published tutorials rebuilt without changes, and the four framework
> acceptance projects.

The framework track ends with one question: can a developer define, train,
evaluate, save, export and deploy a new neural network in Coax and Psionic
without PyTorch? This chapter answers it with records, not adjectives. The
answer has three parts:

- **yes, for a fixed set of small CPU workflows:** an MLP and a CNN on MNIST,
  a tiny decoder language model, checkpoints, Safetensors, DLPack and ONNX in
  both directions. Each one has run and has a recorded result;
- **not yet, for the three published PyTorch tutorials as written:** each one
  uses an operator that the public trainer cannot compose;
- **not yet, as anyone's whole ecosystem:** there is no package for outside
  users, no general GPU training and no multi-machine training.

Three tools keep that answer honest:

- **a model audit:** it reads a model's layer list, dtype, device and sizes,
  and refuses the first thing Psionic cannot train. It allocates nothing;
- **a generated capability matrix:** it is rebuilt from the
  [chapter registry](chapters.json), the capability claims ledger, the audit's
  operator table and the workflow record. A book check fails if any copy
  drifts;
- **a prose check:** it fails if this chapter names a device or chapter that
  the registry does not back.

```shell
coax/bin/coax run psionic/examples/pytorch-audit.coax
python3 psionic/tests/pytorch_matrix.py
psionic/check.sh --only pytorch-audit
```

## Describe a model by its PyTorch layers

PyTorch users already think of a model as its layers, so the audit takes a
model in that form. A description is a list of up to 32 operator numbers,
plus the dtype, the device, the trainable parameter count, the graph node
count and the training rows. Each operator number points into a table of 32
PyTorch operators: the ones the three standard tutorials use (a classifier,
a CNN and a transformer language model) and the common functions around them.

Each row records the furthest level of support Psionic has for that operator:

| Level | Meaning |
| --- | --- |
| `trained` | A public module constructor ran in a recorded learning run ([chapter 118](ch118-one-trainer-for-every-model.md)'s MNIST MLP or CNN) |
| `graph` | A module constructor or tensor-IR operation with derivatives ([chapter 38](ch038-a-tensor-program-becomes-an-execution-plan.md), [chapter 117](ch117-models-are-compositions-of-modules.md)), so chapter 118's generic trainer can compose it |
| `eager` | Only [chapter 5](ch005-a-standard-library-of-tensor-operators.md)'s eager CPU operator library has it, with its gradient; the trainer cannot compose it |
| `none` | No public operator |

<!-- source: psionic/lib/pytorch-audit.coax table -->
```jai
PA_NAME :: [PA_OPS]string.[
    "nn.Linear","nn.ReLU","nn.Conv2d","nn.Flatten","nn.MSELoss",
    "torch.matmul","torch.add","torch.mul","torch.sum","torch.reshape","nn.LayerNorm","nn.Dropout",
    "torch.sub","torch.div","torch.exp","torch.log","torch.mean","torch.amax","torch.cat","torch.topk",
    "nn.Softmax","nn.CrossEntropyLoss","nn.MaxPool2d","nn.AvgPool2d","nn.Embedding",
    "nn.MultiheadAttention","nn.TransformerEncoderLayer","nn.GELU","torch.tanh","torch.sigmoid","torch.sin","nn.BatchNorm2d"];
PA_LEVEL :: [PA_OPS]int.[
    3,3,3,3,3,
    2,2,2,2,2,2,2,
    1,1,1,1,1,1,1,1,
    1,1,1,1,1,
    0,0,0,0,0,0,0];
PA_CHAPTER :: [PA_OPS]int.[
    118,118,118,118,118,
    38,38,38,38,38,117,117,
    5,5,5,5,5,5,5,5,
    5,5,5,5,5,
    0,0,0,0,0,0,0];
PA_PSIONIC :: [PA_OPS]string.[
    "mdl_linear","mdl_relu","mdl_conv2d","mdl_flatten","mdl_squared_error",
    "matmul","add","multiply","sum","reshape","mdl_layernorm","mdl_dropout",
    "subtract","divide","exponential","logarithm","mean","maximum","concat","topk",
    "softmax","cross_entropy","maxpool2d","avgpool2d","take",
    "","","","","","",""];
```

The book check reads these four arrays and checks every row against the
code it names, not just the registry. A `trained` row must name a
constructor that the recorded MLP or CNN model calls. A `graph` row must name
a `TiOp` member or a module constructor. An `eager` row must name an
operator in chapter 5's operator manifest. A `none` row must cite no chapter
and no code. Every cited chapter must be implemented or partly implemented
in the registry.

That gives the first count against a stated list: of 32 PyTorch operators,
5 have trained in a recorded run, 12 can join the trainer's graph, 13 more
exist only as eager operators, and 7 do not exist. The absent ones are
attention, the transformer encoder layer, GELU, tanh, sigmoid, sine and
batch normalization.

## Work one audit by hand

The audit's rule is short. It checks in a fixed order and stops at the first
refusal:

1. The description has between 1 and 32 layers, or it is refused as
   `layers`.
2. Each layer in order: a `none` operator is refused as `op` at that
   layer; an `eager` operator is refused as `graph` at that layer, because the
   trainer cannot compose it.
3. The dtype is f32, or `dtype`. The device is the CPU, or `device`.
4. The trainer's three capacities: 1 to 4,096 trainable values
   (`parameters`), 1 to 32 graph nodes (`nodes`), 1 to 4,096 training rows
   (`rows`). These are the trainer's own constants: `CK_VALUES` in the
   checkpoint record, `TI_NODES` in the tensor IR and `TR_EXAMPLES` in the
   trainer. The book check fails if they differ.

PyTorch's quickstart classifier on FashionMNIST is `Flatten`, `Linear(784,
512)`, `ReLU`, `Linear(512, 512)`, `ReLU`, `Linear(512, 10)`, with
`CrossEntropyLoss`. As table rows that is 3, 0, 1, 0, 1, 0, 21. Walk it:

| Layer | Operator | Level | Result |
| --- | --- | --- | --- |
| 0 | `nn.Flatten` | trained | pass |
| 1 | `nn.Linear` | trained | pass |
| 2 | `nn.ReLU` | trained | pass |
| 3 | `nn.Linear` | trained | pass |
| 4 | `nn.ReLU` | trained | pass |
| 5 | `nn.Linear` | trained | pass |
| 6 | `nn.CrossEntropyLoss` | eager | refused: `graph` at layer 6 |

Chapter 5 computes cross-entropy and its gradient eagerly. The module graph
has no cross-entropy node, which is why chapter 118's MNIST runs used
squared error. Swap in `nn.MSELoss` (row 4) and the operators pass. The
capacities come next. The parameter count is

```text
784*512 + 512 + 512*512 + 512 + 512*10 + 10
  = 401,408 + 512 + 262,144 + 512 + 5,120 + 10 = 669,706
```

669,706 is more than 4,096, so the model is refused as `parameters`. Shrink
it to 3,322 values (chapter 118's MLP size), and the 60,000 FashionMNIST
training rows exceed 4,096: `rows`. With 4,096 rows it passes. The audit has
said exactly what a PyTorch user would need to change, in the order they
would hit it.

The other two tutorials stop sooner. PyTorch's CIFAR-10 CNN is `Conv2d`,
`ReLU`, `MaxPool2d`, and so on. Layer 2, `nn.MaxPool2d`, is eager only, so it
is refused as `graph` at layer 2. Its 456 + 2,416 + 48,120 + 10,164 + 850 =
62,006 parameters would also exceed the limit. The `nn.Transformer` language
model tutorial starts with `nn.Embedding`. Chapter 5's `take` gathers rows by
index, but the graph's `gather` node has its indices fixed when the graph is
built, so a batch of token ids cannot feed it. It is refused as `graph` at
layer 0. Its later layers would also be refused: sine positional encoding and
`nn.TransformerEncoderLayer` are `none`.

Chapter 118's own two models pass. The MLP is `Linear`, `ReLU`, `Linear`,
`MSELoss`: 3,322 values, 17 nodes, 4,096 rows. The CNN is `Conv2d`, `ReLU`,
`Flatten`, `Linear`, `MSELoss`: 2,910 values, 18 nodes, 4,096 rows.

## The audit

<!-- source: psionic/lib/pytorch-audit.coax audit -->
```jai
pa_level :: (op:int)->int #export_c {
    if op<0 || op>=PA_OPS {return 0;}
    return PA_LEVEL[op];
}
pa_count :: ()->PaCount {
    c:PaCount;
    for i:0..<PA_OPS {
        l:=PA_LEVEL[i];
        if l==3 {c.trained+=1;} else if l==2 {c.graph+=1;} else if l==1 {c.eager+=1;} else {c.none+=1;}
    }
    return c;
}
// The first refusal, in a fixed order: the description itself, each layer
// in order, dtype, device, then the trainer's three capacities.
pa_audit :: (m:PaModel)->PaAudit {
    if m.count<1 || m.count>PA_LAYERS {return PaAudit.{status=PaStatus.layers,at= -1};}
    for i:0..<m.count {
        l:=pa_level(m.ops[i]);
        if l==0 {return PaAudit.{status=PaStatus.op,at=i};}
        if l<PA_GRAPH {return PaAudit.{status=PaStatus.graph,at=i};}
    }
    no:=PaAudit.{at= -1};
    if m.dtype!=PaDtype.f32 {no.status=PaStatus.dtype;return no;}
    if m.device!=PaDevice.cpu {no.status=PaStatus.device;return no;}
    if m.parameters<1 || m.parameters>PA_PARAMETERS {no.status=PaStatus.parameters;return no;}
    if m.nodes<1 || m.nodes>PA_NODES {no.status=PaStatus.nodes;return no;}
    if m.rows<1 || m.rows>PA_ROWS {no.status=PaStatus.rows;return no;}
    return no;
}
```

The audit reads a value and returns a value. It touches no tensor, arena or
device, so a model that cannot run is refused before any storage for it
exists. Because the file decides admission, it is marked `#authority`.

## Run it

The [example](../examples/pytorch-audit.coax) prints the table by level and
audits the five descriptions, then the broken ones:

<!-- source: psionic/examples/pytorch-audit.coax example -->
```jai
show :: (name:string,m:PaModel) {
    a:=pa_audit(m);
    if a.at<0 {print("%: %\n",name,a.status);return;}
    if pa_level(m.ops[a.at])==0 && (m.ops[a.at]<0 || m.ops[a.at]>=PA_OPS) {print("%: % at layer % (operator %, not in the table)\n",name,a.status,a.at,m.ops[a.at]);return;}
    print("%: % at layer % (%, %)\n",name,a.status,a.at,PA_NAME[m.ops[a.at]],PA_LEVEL_NAME[pa_level(m.ops[a.at])]);
}
main :: () {
    c:=pa_count();
    print("% PyTorch operators: % trained, % graph, % eager only, % absent\n",PA_OPS,c.trained,c.graph,c.eager,c.none);
    for l:0..<4 {
        print("  %:",PA_LEVEL_NAME[3-l]);
        for i:0..<PA_OPS {if PA_LEVEL[i]==3-l {print(" %",PA_NAME[i]);}}
        print("\n");
    }
    show("chapter 118 MNIST MLP",pa_mlp());
    show("chapter 118 MNIST CNN",pa_cnn());
    show("PyTorch quickstart classifier",pa_quickstart());
    show("PyTorch CIFAR-10 CNN",pa_cifar());
    show("PyTorch nn.Transformer language model",pa_transformer());
    m:=pa_quickstart();m.ops[6]=4;
    show("quickstart with MSELoss",m);
    m.parameters=3322;
    show("  and 3322 parameters",m);
    m.rows=4096;
    show("  and 4096 rows",m);
    // Deliberately broken descriptions: each is refused before any storage exists.
    m=pa_mlp();m.dtype=PaDtype.bf16;show("MNIST MLP in bf16",m);
    m=pa_mlp();m.device=PaDevice.cuda;show("MNIST MLP on CUDA",m);
    m=pa_mlp();m.ops[1]=27;show("MNIST MLP with GELU",m);
    m=pa_mlp();m.ops[1]=99;show("MNIST MLP with operator 99",m);
    m=pa_mlp();m.count=0;show("an empty model",m);
}
```

```shell
coax/bin/coax run psionic/examples/pytorch-audit.coax
coax/bin/coax run psionic/examples/pytorch-audit.coax --release
```

The [transcript](../fixtures/pytorch/pytorch-audit.out) is the same in debug
and release builds, and the program exits with status 0:

```text
32 PyTorch operators: 5 trained, 7 graph, 13 eager only, 7 absent
  trained: nn.Linear nn.ReLU nn.Conv2d nn.Flatten nn.MSELoss
  graph: torch.matmul torch.add torch.mul torch.sum torch.reshape nn.LayerNorm nn.Dropout
  eager: torch.sub torch.div torch.exp torch.log torch.mean torch.amax torch.cat torch.topk nn.Softmax nn.CrossEntropyLoss nn.MaxPool2d nn.AvgPool2d nn.Embedding
  none: nn.MultiheadAttention nn.TransformerEncoderLayer nn.GELU torch.tanh torch.sigmoid torch.sin nn.BatchNorm2d
chapter 118 MNIST MLP: ok
chapter 118 MNIST CNN: ok
PyTorch quickstart classifier: graph at layer 6 (nn.CrossEntropyLoss, eager)
PyTorch CIFAR-10 CNN: graph at layer 2 (nn.MaxPool2d, eager)
PyTorch nn.Transformer language model: graph at layer 0 (nn.Embedding, eager)
quickstart with MSELoss: parameters
  and 3322 parameters: rows
  and 4096 rows: ok
MNIST MLP in bf16: dtype
MNIST MLP on CUDA: device
MNIST MLP with GELU: op at layer 1 (nn.GELU, none)
MNIST MLP with operator 99: op at layer 1 (operator 99, not in the table)
an empty model: layers
```

The last five lines are deliberately broken descriptions. Each is refused
before any storage exists:

- **bf16:** refused as `dtype`;
- **a CUDA device:** refused as `device`. Chapter 129 trains only chapter
  115's decoder on CUDA, not modules built from the public constructors;
- **GELU:** refused as `op` at layer 1;
- **operator 99, outside the table:** refused as `op` at layer 1;
- **an empty layer list:** refused as `layers`.

## How the audit is checked

[pytorch-audit-checks.coax](../tests/pytorch-audit-checks.coax) holds the five
descriptions. It derives each parameter count independently from the
published layer sizes. For example, the transformer tutorial's
12,025,582 parameters are 28,782 × 200 for the embedding, plus 2 × 242,000
for the two encoder layers, plus 200 × 28,782 + 28,782 for the readout. The
`#test` and `#forall` checks pin:

- the table's counts (5, 7, 13, 7);
- that a `none` row has no chapter or name;
- each tutorial's first refusal;
- the refusal order;
- each capacity's bound, one past it, and zero;
- every one of the 36 operator numbers from −2 to 33 as a one-layer model:
  it passes exactly when its level is `graph` or `trained`.

`coax mutate psionic/lib/pytorch-audit.coax` killed 125 of 125 mutants.

## Nine workflows, end to end

The audit answers "can this model train?" The workflows answer "what has
actually run?" Each workflow in the
[workflow record](../fixtures/pytorch/workflows.json) names:

- the PyTorch code it stands in for;
- the chapters that run it;
- the targets it claims;
- evidence lines and values from that chapter's transcripts and run
  records;
- its gaps.

The book check fails if a cited chapter is not runnable, if a claimed target
is not in a cited chapter's `qualified_targets`, or if an evidence line is
not an exact line of its file. Every transcript cited is an example that
`python3 psionic/tests/book.py --coax coax/bin/coax` replays in debug and
release.

<!-- claim: 118 cpu --> The MLP and the CNN train on 4,096 MNIST images on
the CPU with chapter 118's trainer. The MLP reaches 1,755 of 2,048 test
images (85.7%) in 17.0 seconds, and the CNN reaches 1,814 (88.6%) in 13.9
seconds.

<!-- claim: 115 cpu --> Chapter 115's decoder beats its bigram baseline by
0.114 nats per symbol on held-out text and resumes bit for bit.

<!-- claim: 129 cuda --> Chapter 129 trains the same decoder on CUDA within a
tolerance frozen before the first device run. On the one recorded RTX 4080,
its device step is slower than the CPU step: 54.88 against 17.14 ms. No
other model trains on a GPU.

<!-- claim: 8 cpu --> <!-- claim: 220 cpu --> <!-- claim: 221 cpu -->
Chapters 8, 220 and 221 move weights and graphs in and out:
- chapter 8 reads and writes Safetensors and imports and exports DLPack
  CPU tensors;
- chapter 220 reads ONNX opset 17 and runs it;
- chapter 221 writes ONNX opset 17.

All three refuse what they do not cover. Chapter 221, for example, refuses
an MLP with tanh.

<!-- claim: 122 cpu --> <!-- claim: 134 cpu --> <!-- claim: 135 cpu -->
Chapter 122 resumes a run bit for bit. Chapters 134 and 135 split one
training step over logical ranks in one CPU process and match the
single-rank sums bit for bit.

## PyTorch as a comparator, never a dependency

Matching a recorded transcript shows that Psionic repeats itself. It does
not show that Psionic computes what PyTorch would. For the two MNIST
models, [pytorch_parity.py](../tests/pytorch_parity.py) loads each retained
`model.safetensors` into PyTorch 2.11.0. It recomputes the scores for the
2,048 official test images with chapter 118's pooling, in float32 on one
thread. It then compares them with the scores Psionic recorded during the
run. PyTorch is not a repository dependency. The script runs only in an
ephemeral Nix shell, and nothing in Psionic imports it:

```shell
python3 psionic/tests/mnist_reference.py acquire --data-dir /tmp/psionic-mnist-data
nix shell --impure --expr 'let p = (builtins.getFlake "nixpkgs").legacyPackages.${builtins.currentSystem}; in p.python3.withPackages (ps: [ps.torch])' \
  -c python3 -I psionic/tests/pytorch_parity.py /tmp/psionic-mnist-data psionic/fixtures/pytorch/parity.json
```

The [recorded comparison](../fixtures/pytorch/parity.json):

| Model | Predictions that agree | Largest score gap | Scores equal in every bit | Correct, Psionic and PyTorch |
| --- | --- | --- | --- | --- |
| MLP | 2,048 / 2,048 | 5.36 × 10⁻⁷ | 6,114 / 20,480 | 1,755 and 1,755 |
| CNN | 2,048 / 2,048 | 2.98 × 10⁻⁷ | 4,301 / 20,480 | 1,814 and 1,814 |

The gaps come from summation order. Psionic's tensor IR sums binary32
products in a fixed sequence, and PyTorch's matrix kernels choose their own.
Both gaps are within a few binary32 units of the largest scores. The book
check ties this record to the run records: if the comparator's count of
Psionic's correct predictions differs from the run's `final_correct`, it
fails. This compares inference. It does not compare training trajectories,
and it does not time PyTorch.

## The gaps, stated

The workflows above all run on the CPU unless a paragraph says otherwise.
Here is what a PyTorch user would miss, by area:

- **Operator coverage:** 12 of the 32 listed operators can join the
  trainer's graph. Cross-entropy, pooling, softmax and embedding lookups by
  data are eager only. Attention, GELU, tanh, sigmoid, sine and batch
  normalization do not exist. Convolution is stride 1, with no padding and
  one group.
- **Dynamic shapes:**
  [chapter 39](ch039-compile-a-general-tensor-program.md) runs a bounded
  symbolic template whose batch changes without new kernels. A module
  graph's shapes are fixed when it is built, and chapter 220's ONNX reader
  refuses a symbolic dimension.
- **Size:** the trainer holds at most 4,096 trainable values, 4,096
  training rows and 32 graph nodes, so every published tutorial is too
  large.
- <!-- claim: 6 metal --> <!-- claim: 38 metal --> <!-- claim: 129 cuda -->
  **Devices:** tensors and tensor-IR programs run on CPU, CUDA and Metal
  (chapters 6 and 38). Training on a device is limited to chapter 129's
  decoder on CUDA. There is no Metal training.
- **Distributed training:** chapters 133–135 split work over logical ranks
  in one process. No second GPU, no second machine and no failure recovery
  are covered.
- **Autograd:** chapter 106 differentiates bounded CPU tensor programs, and
  the ledger's limits apply. A registered custom operator from
  [chapter 40](ch040-add-your-own-operator.md) is refused by the module
  trainer: `mtr_plan` returns `execution` for a `TiOp.registered` node.
- **Ecosystem:** Psionic is used from Coax. There is no Python API, no
  package outside this repository, no pretrained model hub, no dataset
  library and no third-party module collection.

## The generated matrix

[pytorch_matrix.py](../tests/pytorch_matrix.py) builds this matrix from:

- the registry's entries for this chapter's dependencies;
- three scopes from the capability claims ledger;
- the audit's operator table;
- the workflow record and the parity record.

The same text is in
[matrix.txt](../fixtures/pytorch/matrix.txt). `python3 psionic/tests/book.py`
fails if this block, that file and the regenerated text differ by one byte.
Run `python3 psionic/tests/pytorch_matrix.py --write` after changing any of
those inputs.

<!-- generated: psionic/fixtures/pytorch/matrix.txt -->
```text
Chapters chapter 250 depends on (psionic/book/chapters.json): 23
    3  implemented         cpu             bounded CPU f32 views
    5  implemented         cpu             CPU-f32-wide-views-v1
    6  implemented         cpu,cuda,metal  bounded CPU, CUDA and Metal
    8  implemented         cpu             interchange
   38  implemented         cpu,cuda,metal  tensor-ir-f32-v1, CPU/CUDA/Metal
   39  implemented         cpu             tensor-template
   40  implemented         cpu             operator-registry
  106  implemented         cpu             autodiff-f32-v1, bounded CPU
  114  partly implemented  cpu             compiled-backward-f32-v1, bounded CPU
  115  implemented         cpu             tiny-train-cpu-f64-v1, bounded CPU
  117  partly implemented  cpu             module-f32-v1, bounded CPU
  118  partly implemented  cpu             trainer-cpu-f64-v1, bounded CPU
  121  partly implemented  cpu             psionic-feed-v1 on psionic-pack-v1, bounded CPU
  122  implemented         cpu             psionic-resume-v1, bounded CPU
  124  implemented         cpu             cpu-adamw-v1, cpu-muon-v1
  129  partly implemented  cpu,cuda        gpu-train-f64-v1, CPU and CUDA
  130  partly implemented  cpu,cuda        tp-step-v1, CPU and CUDA
  131  partly implemented  cpu             train-compare-v1, bounded CPU
  133  partly implemented  cpu             shard-v1, bounded CPU logical devices
  134  partly implemented  cpu             data-parallel-v1, bounded CPU ranks
  135  partly implemented  cpu             fully-sharded-v1, bounded CPU logical ranks
  220  implemented         cpu             onnx-17-v1, bounded CPU
  221  implemented         cpu             onnx-17-write-v1, bounded CPU
  implemented 13, partly implemented 10, design draft 0

Limits in the capability claims ledger (coax/book/capability-claims.json):
  psionic.autodiff.current: Bounded CPU scalar/tensor autodiff and permanent-IR function transforms through order two; recorded tiny-decoder CUDA backward/optimizer/resume. No arbitrary Coax transforms, general GPU backward coverage, efficient cooperative training or mixed-precision qualification.
  psionic.execution.current: Retained named model/device profiles and bounded CPU IR/tiny-decoder CUDA training profiles; Mammoth Metal, original120b GPU, complete V4.1 and general efficient GPU training remain unqualified.
  psionic.gpu.current: Recorded original20b CUDA/Metal and bounded resident CUDA serving profiles; broader model/device/serving work remains open.

PyTorch operators (psionic/lib/pytorch-audit.coax): 32; trained 5, graph 7, eager only 13, absent 7
  nn.Linear                     trained  chapter 118, mdl_linear
  nn.ReLU                       trained  chapter 118, mdl_relu
  nn.Conv2d                     trained  chapter 118, mdl_conv2d
  nn.Flatten                    trained  chapter 118, mdl_flatten
  nn.MSELoss                    trained  chapter 118, mdl_squared_error
  torch.matmul                  graph    chapter 38, matmul
  torch.add                     graph    chapter 38, add
  torch.mul                     graph    chapter 38, multiply
  torch.sum                     graph    chapter 38, sum
  torch.reshape                 graph    chapter 38, reshape
  nn.LayerNorm                  graph    chapter 117, mdl_layernorm
  nn.Dropout                    graph    chapter 117, mdl_dropout
  torch.sub                     eager    chapter 5, subtract
  torch.div                     eager    chapter 5, divide
  torch.exp                     eager    chapter 5, exponential
  torch.log                     eager    chapter 5, logarithm
  torch.mean                    eager    chapter 5, mean
  torch.amax                    eager    chapter 5, maximum
  torch.cat                     eager    chapter 5, concat
  torch.topk                    eager    chapter 5, topk
  nn.Softmax                    eager    chapter 5, softmax
  nn.CrossEntropyLoss           eager    chapter 5, cross_entropy
  nn.MaxPool2d                  eager    chapter 5, maxpool2d
  nn.AvgPool2d                  eager    chapter 5, avgpool2d
  nn.Embedding                  eager    chapter 5, take
  nn.MultiheadAttention         none     -
  nn.TransformerEncoderLayer    none     -
  nn.GELU                       none     -
  torch.tanh                    none     -
  torch.sigmoid                 none     -
  torch.sin                     none     -
  nn.BatchNorm2d                none     -

Workflows (psionic/fixtures/pytorch/workflows.json): 9
1. MLP classifier on MNIST
   PyTorch: nn.Sequential(nn.Linear, nn.ReLU, nn.Linear), an optimizer and a training loop
   chapters: 106 implemented, 117 partly implemented, 118 partly implemented, 124 implemented; targets: cpu
   psionic/fixtures/mnist-classifier.out: MNIST MLP: 196 -> 16 -> 10; 3322 parameters, 17 nodes
   psionic/fixtures/mnist/run-v1/result.json: final_correct = 1755
   psionic/fixtures/mnist/run-v1/result.json: test_rows = 2048
   psionic/fixtures/mnist/run-v1/result.json: seconds = 17.036464
   PyTorch parity: 2048 of 2048 predictions agree, largest score gap 5.36e-07
   not yet: the loss is squared error: cross-entropy is eager only, not in the module graph
   not yet: at most 4,096 trainable values, 4,096 training rows and 32 graph nodes
   not yet: 4,096 training images and a 2,048-image test slice, not the full dataset
   not yet: CPU only
2. CNN classifier on MNIST
   PyTorch: nn.Conv2d, nn.ReLU, nn.Flatten, nn.Linear and a training loop
   chapters: 117 partly implemented, 118 partly implemented; targets: cpu
   psionic/fixtures/cnn-classifier.out: input [1,1,14,14], conv [2,1,3,3], activation [1,2,12,12], flatten [1,288], scores [1,10]
   psionic/fixtures/cnn/run-v1/result.json: final_correct = 1814
   psionic/fixtures/cnn/run-v1/result.json: seconds = 13.895613
   PyTorch parity: 2048 of 2048 predictions agree, largest score gap 2.98e-07
   not yet: convolution is stride 1, no padding, one group
   not yet: no pooling or batch normalization in the module graph
   not yet: CPU only: convolution gradients do not run on a GPU
3. Tiny decoder language model
   PyTorch: a small decoder-only transformer trained on text, sampled and resumed
   chapters: 115 implemented; targets: cpu
   psionic/fixtures/tiny-train.out: held-out: decoder 2.1320579221135967, bigram 2.245995271590946, decoder better by 0.11393734947734924 (frozen margin 0.05)
   psionic/fixtures/tiny-train.out: resumed from step 350 (75908 bytes): 50 of 50 losses identical, weights and moments identical: true
   not yet: the decoder is written by hand for this chapter: attention and embedding are not public modules
   not yet: 3,152 parameters and a 4,080-symbol corpus
4. The same decoder on one GPU
   PyTorch: model.to("cuda") and the same training loop
   chapters: 129 partly implemented; targets: cpu, cuda
   psionic/fixtures/gpu-train.out: losses, parameters and moments equal chapter 118's run bit for bit: true
   psionic/book/ch129-train-a-whole-model-on-one-gpu.md: chapter 129 on CUDA: the decoder trains on the device within the frozen tolerance of chapter 118's CPU run, and resumes bit for bit
   psionic/book/ch129-train-a-whole-model-on-one-gpu.md: receipt, 64 rows a step (16114648 bytes on the device): device 54.88 ms/step (37320 tokens/s), CPU 17.14 ms/step (119462 tokens/s); parameters 0 past the tolerance
   not yet: one recorded device, an RTX 4080; the device step is slower than the CPU step
   not yet: only chapter 115's decoder trains on the device; no Metal training
5. Safetensors and DLPack exchange
   PyTorch: safetensors.torch.save_file, load_file and torch.from_dlpack
   chapters: 8 implemented; targets: cpu
   psionic/fixtures/model-interchange.out: Portable parameter: scale.weight, F32 [1], 4 bytes
   psionic/fixtures/model-interchange.out: Reloaded prediction at x=3: 5.999999
   psionic/fixtures/interchange-profile.json: dlpack.device = CPU:0
   not yet: Safetensors: F32 and BF16, at most 16 tensors of rank 4 or less
   not yet: DLPack: contiguous CPU f32 tensors only
6. ONNX in
   PyTorch: a model exported with torch.onnx.export, then run
   chapters: 220 implemented; targets: cpu
   psionic/fixtures/onnx-read.out: classifier.onnx: 1175 bytes, IR 8, opset 17: 5 nodes, 4 weights, 257 floats of arena
   psionic/fixtures/onnx-read.out: image 0 -> 0.715006 0.081689 0.203305
   not yet: opset 17 only, seven operators: Gather, ReduceMean, Conv, Relu, Flatten, Gemm, Softmax
   not yet: static shapes only: a symbolic dimension is refused
7. ONNX out
   PyTorch: torch.onnx.export
   chapters: 221 implemented; targets: cpu
   psionic/fixtures/onnx-write.out: deadline.onnx: Gemm, then Softmax over [0, z]
   psionic/fixtures/onnx-write.out: chapter 103's MLP: op, tanh is outside the subset
   not yet: Gemm and Softmax only: a model with tanh is refused
8. Checkpoint and resume
   PyTorch: torch.save and torch.load of the model, optimizer and data order
   chapters: 122 implemented; targets: cpu
   psionic/fixtures/resume.out: loss 2.772588722239781 -> 1.4089266754480803; after resuming at 5, 7 of 7 losses and all 768 values are identical
   not yet: 4,096 values at most
9. Data-parallel and fully sharded training
   PyTorch: DistributedDataParallel and FullyShardedDataParallel
   chapters: 133 partly implemented, 134 partly implemented, 135 partly implemented; targets: cpu
   psionic/fixtures/data-parallel.out: hand example, 2 ranks: rank 0 sends 0.375 -0.5 2.25; the sum is 0.875 4.0 1.75, loss 5.0; equal to tr_add bit for bit: true
   psionic/fixtures/fully-sharded.out: 1 ranks: parameters, moments and losses equal bit for bit: true; 0 bytes moved a step (gather 0, reduce-scatter 0)
   not yet: logical ranks in one CPU process: no second GPU, no second machine, no failure recovery
```

The same check reads this chapter's prose:

- each `claim` marker must name a chapter that the registry marks runnable
  and that qualified that target;
- a paragraph that names CUDA, Metal or a GPU without such a marker must
  say what is missing;
- a link to a design draft must say that it is a draft.

The check binds claims to records; it does not prove the prose true.

## How it is checked

```shell
coax/bin/coax test psionic/lib/pytorch-audit.coax
coax/bin/coax test psionic/lib/pytorch-audit.coax --release
coax/bin/coax mutate psionic/lib/pytorch-audit.coax
python3 psionic/tests/pytorch_matrix.py
python3 psionic/tests/book.py
```

## What this does not do yet

- **Outside users:** no package with public API documentation, outside
  examples and reproducible benchmarks.
- **The three published tutorials:** they are not yet rebuilt with public
  APIs alone. The audit shows where each one stops: cross-entropy, max
  pooling and token embedding first, then the trainer's size limits.
- **Engine changes and effort:** model-specific lines, engine
  modifications, cold-start compile time, throughput and memory against a
  PyTorch run are not yet recorded per tutorial.
- **The four acceptance projects:** train from scratch at a useful size, a
  language model without PyTorch, import, fine-tune and serve, and training
  across GPUs with recovery all remain.
- **Device audits:** the audit refuses every device except the CPU. It does
  not yet check GPU memory against a device's free memory.

Each of these needs a missing public abstraction, and that abstraction gets
added before this chapter is marked implemented. This chapter does not
claim that Psionic replaces PyTorch as anyone's ecosystem.

## Dependencies

- [Chapter 3](ch003-a-tensor-is-a-view-of-memory.md), [Chapter 5](ch005-a-standard-library-of-tensor-operators.md) and [Chapter 6](ch006-one-tensor-several-devices-and-dtypes.md): tensors, operators and devices.
- [Chapter 38](ch038-a-tensor-program-becomes-an-execution-plan.md), [Chapter 39](ch039-compile-a-general-tensor-program.md) and [Chapter 40](ch040-add-your-own-operator.md): the tensor IR, compilation and custom operators.
- [Chapter 106](ch106-differentiate-any-tensor-program.md), [Chapter 114](ch114-compile-the-backward-pass.md), [Chapter 115](ch115-train-the-whole-tiny-decoder.md), [Chapter 117](ch117-models-are-compositions-of-modules.md), [Chapter 118](ch118-one-trainer-for-every-model.md), [Chapter 121](ch121-feed-the-accelerator.md) and [Chapter 124](ch124-adamw-and-explicit-precision.md): autodiff, the tiny decoder, modules, the trainer, data loading and optimizers.
- [Chapter 129](ch129-train-a-whole-model-on-one-gpu.md), [Chapter 130](ch130-profile-a-training-step.md) and [Chapter 131](ch131-train-the-same-model-eager-and-compiled.md): GPU training and profiling. All three are only partly implemented.
- [Chapter 8](ch008-models-travel-between-frameworks.md), [Chapter 122](ch122-a-checkpoint-holds-the-whole-experiment.md), [Chapter 220](ch220-read-a-bounded-onnx-graph.md) and [Chapter 221](ch221-write-a-bounded-onnx-graph.md): checkpoints and interchange.
- [Chapter 133](ch133-a-tensor-spread-over-devices.md), [Chapter 134](ch134-data-parallel-training.md) and [Chapter 135](ch135-fully-sharded-training.md): distributed training on logical CPU ranks; multi-GPU training is not yet covered.

## Sources

- [Chapter plan](PLAN.md), Part XXIV and "A framework, not only engines".
