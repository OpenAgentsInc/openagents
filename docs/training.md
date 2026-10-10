# Training your own Jev-style classifiers on consumer hardware

> **In this repository.** This is an outside research report (October 10, 2026) on training Jev-style decision models on a Mac or an NVIDIA PC with upstream Kev, Unsloth, and Clef. What we have built so far: `crates/kev` serves our pinned Kev checkpoints ([Kev](kev/README.md)), `psionic-serve` runs Clef-Flash at `/v1/systemone` ([native Clef](inference/clef-native.md)), and the [Gym](gym/README.md) decides which models get admitted. Our own trainer does not exist yet. The [training system audit](audits/2026-10-10-training-system-audit/README.md) and its [roadmap](audits/2026-10-10-training-system-audit/roadmap.md) set the order: calibrate Clef-Flash (X1), then a `decision-train` lane in `psionic-train` (X2). The recipes below are the outside baseline that lane has to match. Upstream Kev's variant names here (0.8B/4B/9B/27B) are newer than the checkpoints our port pins (`kev-0.5b`, `kev-0.6b`, `kev-4b`, `kev-8b`).

**Yes—you can train useful Jev-style decision models on an Apple Silicon Mac or an NVIDIA gaming PC.** You do not need to pretrain a language model or reproduce Jev’s proprietary training process.

The most practical routes are:

| Your goal | My recommended starting point |
|---|---|
| Adapt an existing Jev-like model to your own questions and policies | **Fine-tune Kev-0.8B or Kev-4B** |
| Train a decision model with a custom classification head on NVIDIA | **Unsloth’s `FastDecisionModel` and `DecisionTrainer`** |
| Train that kind of model natively on Apple Silicon through MLX | **Unsloth’s current MLX decision-training backend** |
| Classify into a small, mostly fixed set of categories | **SetFit or a small Hugging Face sequence-classification model** |
| Experiment specifically with Cloudflare’s architecture | **Fine-tune Clef through a compatible decision-model trainer, or attach a Clef-style head to a smaller backbone** |

An important current development: **Unsloth now has actual MLX decision-training code—not merely Mac inference support.** Kev also supports MLX, but its own training program is PyTorch-based. Those are distinct capabilities.   

This report reflects the documentation and source available on **October 10, 2026**. I reviewed the implementations but did not run GPU training benchmarks. Hardware recommendations below are planning estimates unless explicitly attributed to a published experiment.

**Not included here:** the original report shipped a `jev_classifier_starter.zip` (a Python training script, example data, and setup notes; syntax-checked, not hardware-tested). That archive is not in this repository, so the commands below that use `train_decisions.py` describe it rather than point at a file you can run.

---

## 1. What distinguishes a Jev-style classifier?

### Fixed-label classification versus instruction-conditioned decisions

A conventional classifier learns a fixed mapping:

```text
Customer message → billing / shipping / technical support
```

Its output layer might have three permanently assigned positions.

A Jev-style model instead receives the decision specification at runtime:

```text
State:
  “I was charged twice. Please refund the duplicate today.”

Questions:
  Which team should handle this?
    billing: invoices, payments, refunds
    technical: bugs, outages, errors
    sales: pricing and new plans

  Does the customer request a refund?

  What response timing is requested?
    no deadline / soon / today
```

It returns probabilities over the supplied alternatives. The same model can receive different categories, instructions, and numbers of options on the next request. Kev and Clef implement this schema-conditioned behavior using specialized decision heads rather than ordinary text generation.  [Hugging Face](https://huggingface.co/Cloudflare/clef)

The conceptual pipeline is:

```text
State + instructions + candidate answers
                  │
        Pretrained language backbone
                  │
          Contextual representations
                  │
         Specialized decision head
                  │
       Scores for the supplied options
                  │
        Probability calibration
                  │
        Typed answers + probabilities
```

The important distinction is **not simply “small LLM.”** It is that the model directly scores decisions instead of repeatedly generating tokens until it finishes an answer.

That removes the text-decoding loop, but it does not make processing a long document free. The backbone still has to process the input. Cloudflare’s published architecture explicitly separates backbone processing from a comparatively small joint decision head. [Cloudflare Blog](https://blog.cloudflare.com/clef-decision-models/)

### What you actually train

For a question with candidate scores $z_1,\ldots,z_K$, the simplest training objective is:

$$
p_i=\frac{e^{z_i}}{\sum_j e^{z_j}},
\qquad
\mathcal{L}=-\log p_y
$$

Here, $y$ is the correct candidate.

On consumer hardware, the usual approach is to freeze most backbone weights and train **LoRA adapters plus the decision head**. QLoRA additionally stores much of the frozen backbone in a quantized representation, reducing its memory requirement. Hugging Face’s PEFT documentation covers this quantized-adapter workflow. [Hugging Face](https://huggingface.co/docs/peft/en/developer_guides/quantization)

For typed decisions:

| Type | Training target | Interpretation |
|---|---|---|
| `choice` | Correct option key | One selected candidate |
| `noul` | Boolean or soft binary target | Probability of “true” |
| `score` | An ordered category | Distribution over ordered levels; the API may also return an expected numerical score |

Kev supports cross-entropy, soft targets, and optional loss modifications, including an ordinal term for score questions. You do **not** need reinforcement learning to begin. 

### Are you reproducing Jev itself?

Not exactly. TypeSafe describes Jev and its training approach publicly, but that is not the same as publishing a complete, independently reproducible internal implementation. The architectural reconstruction that inspired Kev is explicitly an inference from observed behavior, not a verified disclosure of Jev’s internals. [TypeSafe AI](https://typesafe.ai/blog/introducing-system-one-models-and-jev)

A better objective is:

> Build a model with the same useful interface and decision behavior, then measure its performance on your own workload.

---

## 2. Review of Kev: the strongest starting point for adapting an existing model

### What Kev provides

Kev is unusually complete for this purpose: pretrained decision models, a training program, structured training data, evaluation suites, calibration tools, a local server, and compatibility with TypeSafe’s System One interface. The released family includes:

| Model | Backbone | Released training approach |
|---|---|---|
| Kev-0.8B | Qwen3.5-0.8B-Base | Frozen backbone, LoRA, pointer head |
| Kev-4B | Qwen3.5-4B-Base | Frozen backbone, LoRA, pointer head |
| Kev-9B | Qwen3.5-9B-Base | Frozen backbone, LoRA, pointer head |
| Kev-27B | Qwen3.8-27B | Full-weight fine-tuning |

For desktop training, the first two are the relevant starting points. The 27B model’s released training procedure is not a sensible first project on a gaming PC. 

### Architecture: shared state, separate question branches

Kev uses a **pointer-style head** to score the options represented in the input. It does not require a different permanently sized output layer for every customer’s category list.

Its question branches are designed to remain isolated: one question can read the shared state but not another question’s branch. That is useful when, for example, adding a sentiment question should not change a billing classification. The physical implementation varies by backbone and backend; “shared state” does not imply that every training configuration processes that prefix only once. The training code includes a separate shared-prefix optimization.  

### Why warm-starting matters

When adapting Kev, use `--init_from`.

That loads the existing decision model’s adapter and pointer head rather than beginning with an untrained decision head on a generic language model. Kev checks compatibility of the base model, adapter rank, head configuration, and related architecture fields. 

My recommendation is to **adapt an existing Kev checkpoint before attempting to reproduce its broad, multitask training corpus**. Your first experiment should answer whether the existing model can learn your particular policies—not whether you can build a general-purpose decision foundation model.

### Two important implementation traps

**First: BF16 computation and BF16 weight storage are different switches.**

In Kev:

```text
--dtype bf16
```

enables CUDA autocasting, while:

```text
--weights_dtype bf16
```

changes the frozen backbone’s storage precision. The latter defaults to FP32. Setting only the first flag can therefore consume substantially more weight memory than you expected. The adapter and head remain FP32 in the documented LoRA path. 

**Second: MLX serving does not mean Kev trains through MLX.**

Kev’s Mac backend uses an MLX backbone under its existing encoder and pointer-head interface. Its training program remains PyTorch-based; MPS is a separate path. A CUDA-trained Kev adapter can subsequently be loaded for MLX serving, which is a useful train-on-PC, deploy-on-Mac arrangement. 

---

## 3. Review of Cloudflare Clef: similar interface, different architecture

Cloudflare’s original Clef release describes **Clef, based on Qwen3.8-27B, and Clef-flash, based on Qwen3.5-9B**. Its training description uses frozen backbones with rank-256 LoRA, a specialized head, and a combination of label-smoothed cross-entropy and Brier loss. [Cloudflare Blog](https://blog.cloudflare.com/clef-decision-models/)

### Clef’s head considers the questions jointly

Unlike Kev’s isolated question branches, Clef’s head includes evidence-routing and cross-field processing.

Conceptually:

```text
Backbone representations
          │
Question- and option-specific evidence
          │
  Joint processing across fields
          │
     Per-option decision scores
```

That can help decisions inform one another. It also means **you should not assume that adding or removing an unrelated question leaves another answer unchanged**. Test the exact question bundle you will deploy. [Cloudflare Blog](https://blog.cloudflare.com/clef-decision-models/)

Neither architecture is universally preferable. My interpretation is that isolation is attractive for independent reusable checks, while joint processing is attractive when several output fields genuinely constrain one another.

### What the public release lets you reproduce

The Hugging Face release includes backbone weights, a separate joint-head checkpoint and configuration, and custom modeling code. Those are sufficient to load the released model and provide a basis for further training.

However, I did not find a complete public release of Cloudflare’s internal training corpus and every step needed to recreate the original model from its starting checkpoint. **Fine-tuning Clef is different from reproducing Cloudflare’s entire training run.** [Hugging Face](https://huggingface.co/Cloudflare/clef)

Also, this is not an ordinary:

```python
AutoModelForSequenceClassification(...)
```

checkpoint. The published inference path uses its custom model and head. Simply fine-tuning Qwen to emit labels and saving its language-model weights would not preserve Clef’s architecture. [Hugging Face](https://huggingface.co/Cloudflare/clef)

### How I would train with Clef’s architecture

There are two practical choices.

**Adapt the released Clef-flash model** when its existing capabilities justify the 9B backbone. Preserve its trained head and use a compatible fine-tuning loader.

**Attach a fresh Clef-style head to a smaller backbone** when local training cost matters more. Unsloth now implements this route, including an MLX version. Its source freezes the backbone initially, allows the joint head to train, and adds trainable adapters through `get_peft_model`.  

The second option is especially relevant to your question: **you can use the decision-head architecture without committing to a 9B or 27B model.**

---

## 4. What consumer hardware is sufficient?

### Start with the weight-memory floor

Ignoring overhead, the arithmetic is:

$$
\text{weight memory}=\text{parameter count}\times\text{bytes per parameter}
$$

| Backbone size | BF16 weights alone | Idealized 4-bit weights alone |
|---|---:|---:|
| 0.8B | 1.6 GB | 0.4 GB |
| 4B | 8 GB | 2 GB |
| 9B | 18 GB | 4.5 GB |
| 27B | 54 GB | 13.5 GB |

These are **lower bounds, not training requirements**. Quantization metadata, unquantized layers, the decision head, activations, adapters, gradients, optimizer state, and runtime buffers all add memory. QLoRA reduces frozen-weight storage; it does not make those other allocations disappear. [Hugging Face](https://huggingface.co/docs/peft/en/developer_guides/quantization)

### My practical planning recommendations

These assume short initial inputs—roughly 512–2,048 tokens—small microbatches, and gradient checkpointing.

| Machine | Sensible first training target | Assessment |
|---|---|---|
| NVIDIA GPU with 6–8 GB VRAM | Small encoder or 0.8B decision model | A useful starting machine |
| NVIDIA GPU with 12–16 GB VRAM | 0.8B–4B with an optimized quantized trainer | Practical for domain classifiers |
| NVIDIA GPU with 24–32 GB VRAM | 4B comfortably targeted; explore 7–9B QLoRA | My preferred general-purpose range |
| Apple Silicon with 16 GB unified memory | Small encoder or approximately 0.8B MLX experiment | Start small and monitor peak memory |
| Apple Silicon with 24–32 GB | 1–4B MLX adapter training | A sensible development configuration |
| Apple Silicon with 48–64 GB | 4B comfortably targeted; explore 7–9B | More room for context and experimentation |
| Apple Silicon with 96–128 GB | Larger experiments where the backend supports them | Not necessary for a first useful classifier |

These are my estimates, not guaranteed fit claims. In particular, **Mac unified memory is shared with the operating system and applications; it is not equivalent to the same amount of dedicated GPU VRAM.**

There is published evidence that the entry point can be quite low: Unsloth reports a **4 GB VRAM** training configuration for Qwen3.5-0.8B with a decision head. That is evidence for its particular configuration, not a promise that every dataset or sequence length fits in 4 GB. [unsloth.ai](https://unsloth.ai/docs/basics/train-your-own-decision-model-with-unsloth)

### The settings that most affect whether training fits

My starting configuration would be:

```text
Backbone:              0.8B or 4B
Frozen-weight format:  4-bit when supported
LoRA rank:             16
Microbatch:            1
Gradient accumulation: 8
Maximum input length:  1,024 tokens
Gradient checkpointing: enabled
```

For an out-of-memory error, first reduce the microbatch or context length. Accumulation lets you retain a larger effective batch while processing fewer records at a time; it does not eliminate the activation memory of one long record. Unsloth’s decision-training documentation recommends this batch/accumulation tradeoff. [Unsloth - Train and Run Models Locally](https://unsloth.ai/docs/basics/train-your-own-decision-model-with-unsloth)

**I would not buy a larger machine until a small model has failed on a properly evaluated workload.** Better examples and clearer policies may matter more than moving from 4B to 9B.

---

## 5. Build the dataset before choosing the final model

### A useful record contains the decision specification and its answer

For Unsloth’s decision trainer, an illustrative record is:

```json
{
  "state": "I was charged twice. Please refund the duplicate today.",
  "questions": {
    "team": {
      "type": "choice",
      "instructions": "Which team should handle this?",
      "criteria": {
        "billing": "Invoices, payments, refunds",
        "technical": "Bugs, outages, errors",
        "sales": "Pricing and new plans"
      }
    },
    "refund": {
      "type": "noul",
      "instructions": "Does the customer ask for a refund?"
    },
    "urgency": {
      "type": "score",
      "instructions": "What response timing is requested?",
      "criteria": ["No deadline", "Soon", "Today"]
    }
  },
  "gold": {
    "team": "billing",
    "refund": true,
    "urgency": 2
  }
}
```

Save one compact JSON object per line. The current Unsloth parser supports these scalar gold labels as well as probability targets. 

**Kev uses a slightly different training format:** put `"label"` inside each question instead of using the top-level `"gold"` object. Its choice label is an option key, its `noul` label is a boolean, and its score label is a zero-based level. 

The conversion is straightforward:

```python
import copy

kev_record = copy.deepcopy(unsloth_record)
gold = kev_record.pop("gold")

for name, question in kev_record["questions"].items():
    question["label"] = gold[name]
```

### How much data?

My suggested planning targets—not universal sample-complexity guarantees—are:

| Stage | Suggested objective |
|---|---|
| Initial evaluation | A few hundred representative, carefully labeled cases |
| First domain fine-tune | Roughly 1,000–5,000 varied records |
| More dependable deployment | Expand toward 5,000–20,000 as error analysis reveals gaps |
| General-purpose decision model | A broad mixture of tasks, policies, schemas, and genuinely held-out task families |

A thousand near-identical synthetic examples are not equivalent to a thousand independent real cases.

For a first project, I would prioritize **real examples, hard negatives, ambiguous cases, and decision boundaries** over a huge automatically generated corpus.

### Label the policy, not the vibe

Compare:

```text
“Is this urgent?”
```

with:

```text
“Mark urgent when the customer reports complete loss of service,
an active security incident, or a payment deadline within 24 hours.
Angry wording alone is insufficient.”
```

The second definition gives annotators and the model a much clearer target.

Also distinguish:

```text
false
unknown
not applicable
needs human review
```

Do not automatically label insufficient evidence as `false`. An explicit choice question with an “insufficient information” option may better represent the real decision.

### Split before augmentation

I recommend four partitions:

```text
train        → optimize weights
validation   → select model and hyperparameters
calibration  → fit confidence scaling and operating thresholds
test         → final untouched evaluation
```

Split by the unit that could leak information: customer, document, conversation, source template, policy family, or time period. Then generate paraphrases and other augmentations **within** the training partition.

Useful augmentations include reordering options with correctly remapped labels, adding plausible distractors, paraphrasing instructions, and constructing examples just above and below a policy threshold. Kev’s source includes option permutation, distractor, and “none” variants, along with optional consistency losses. 

For a model intended to handle new schemas, ordinary random example splitting is insufficient: hold out entire categories or task families as well.

---

## 6. Concrete workflow A: fine-tune Kev on NVIDIA

### Install from the repository

**Do not run `pip install kev`.** The repository warns that the PyPI name belongs to an unrelated package. Use Python 3.12 or 3.13 and the repository installation path. 

```bash
git clone https://github.com/jaredpalmer/kev.git
cd kev
uv sync --extra serve
```

Prepare your files in Kev’s label-inside-question format.

### Adapt Kev-4B

This follows the repository’s warm-start workflow, with explicit BF16 backbone storage:

```bash
uv run python -m kev.train \
  --data train.jsonl \
  --base Qwen/Qwen3.5-4B-Base \
  --init_from jaredpalmer/kev-4b@v1.0 \
  --epochs 2 \
  --lr 2e-5 \
  --batch 1 \
  --accum 8 \
  --weights_dtype bf16 \
  --dtype bf16 \
  --checkpointing 1 \
  --device cuda \
  --max_state 1024 \
  --out runs/my-classifier
```

The precision, checkpointing, warm-start, and context flags are present in the current training source. This is **BF16 LoRA, not 4-bit QLoRA**. 

For a smaller first run, change both model references:

```text
--base Qwen/Qwen3.5-0.8B-Base
--init_from jaredpalmer/kev-0.8b@v1.0
```

Inspect the training log for discarded records. Kev filters examples that exceed the configured training context; silently losing the long or difficult examples would distort your experiment. 

### Evaluate and calibrate separately

First evaluate a held-out calibration partition:

```bash
uv run python -m kev.benchmark \
  --run runs/my-classifier \
  --data calibration.jsonl \
  --out runs/my-classifier-calibration
```

Then fit and inspect a temperature:

```bash
uv run python -m kev.calibrate \
  --rows runs/my-classifier-calibration/rows.json \
  --out runs/my-classifier-calibration/calibration.json
```

**Important:** `kev.calibrate` produces a report; it does not modify the checkpoint. Its output contains `workload_temperature`, and it also provides an out-of-fold assessment of calibration. 

Load that fitted temperature for serving:

```bash
export KEV_TEMPERATURE="$(
  uv run python -c \
  'import json; print(json.load(open("runs/my-classifier-calibration/calibration.json"))["workload_temperature"])'
)"

uv run --extra serve python -m kev.serve \
  --run runs/my-classifier \
  --port 8009
```

Kev supports this temperature override through its checkpoint loading options. Evaluate the untouched test partition using the same override, and verify the exact backend and precision you will deploy. 

---

## 7. Concrete workflow B: train a Clef-style model on CUDA or native MLX

### Unsloth now supplies the missing training machinery

Its current implementation can attach a Clef-style joint head to a supported language model, train adapters and the head, calibrate predictions, and save the decision artifacts. The Apple Silicon implementation exposes the same main abstractions—`FastDecisionModel` and `DecisionTrainer`—while delegating training to MLX.  

That changes the Mac recommendation substantially: **you no longer have to port Kev’s custom head yourself merely to experiment with genuine decision-head training in MLX.**

### Easiest setup: the current desktop application

For Mac, install the current Unsloth Desktop application, or follow the official Studio installer. The installation documentation explicitly states that MLX training is supported. [Unsloth - Train and Run Models Locally](https://unsloth.ai/docs/get-started/install/mac)

The documented decision-model workflow is:

```text
Train
  → Select a supported language model
  → Train as: Decision model
  → Supply the decision dataset
  → Train
  → Evaluate and calibrate
  → Use in Decision API
```

Start with a small backbone and short inputs rather than selecting the largest model that can be downloaded. The decision-training guide provides the UI workflow and the corresponding Python API. [unsloth.ai](https://unsloth.ai/docs/basics/train-your-own-decision-model-with-unsloth)

### Code-based training

The essential model setup is:

```python
from unsloth import FastDecisionModel, DecisionTrainer

model, tokenizer = FastDecisionModel.from_pretrained(
    model_name="unsloth/Qwen3.5-4B",
    max_seq_length=1024,
    load_in_4bit=True,
)

model = FastDecisionModel.get_peft_model(
    model,
    r=16,
    lora_alpha=16,
    lora_dropout=0,
    use_gradient_checkpointing="unsloth",
    random_state=42,
)
```

Those operations are implemented in the MLX backend as well: loading a language model with a new joint head, quantized loading, and adding trainable adapters. Some advanced LoRA options remain unsupported there, so start with the basic configuration rather than assuming complete CUDA feature parity.  

### Downloadable end-to-end starter

The starter project from the original report (not in this repository) handles loading four separate data splits, building the training records, fitting adapters and the head, calibrating, evaluating, and saving.

Run it **inside a working, current Unsloth training environment**:

```bash
python train_decisions.py \
  --data ./data \
  --out ./my-decisions \
  --model unsloth/Qwen3.5-4B \
  --max-length 1024 \
  --batch 1 \
  --accum 8
```

Expected files:

```text
data/
  train.jsonl
  valid.jsonl
  calibration.jsonl
  test.jsonl
```

For a smaller experiment:

```bash
python train_decisions.py \
  --data ./data \
  --out ./my-small-decisions \
  --model Qwen/Qwen3.5-0.8B-Base
```

The script is intended for a **plain language-model starting checkpoint**. Do not blindly apply it to an already-adapted decision checkpoint: adding another adapter to a checkpoint that already contains one can be incorrect or rejected.

It also checks exact-state overlap across splits and rejects records reported as skipped or truncated. That does not replace a semantic leakage audit.

### Mac-specific details

**Do not force CUDA precision flags on the Mac.** The MLX decision loader rejects explicit dtype overrides for its Clef path and trains at the checkpoint’s supported precision. The starter leaves model dtype unspecified and does not enable CUDA FP16/BF16 training flags on Apple Silicon. 

**Keep the custom head and calibration with the adapter.** A language-model adapter alone is not the complete classifier. Unsloth’s decision save path preserves the necessary decision artifacts; its MLX implementation also provides a merged-model export path. 

**Use current packages together.** The MLX decision module explicitly checks for supporting functionality in `unsloth-zoo` and reports when that dependency is too old. 

### What about plain `mlx-lm`?

Apple’s MLX-LM project has a documented LoRA/QLoRA trainer:

```bash
pip install "mlx-lm[train]"

mlx_lm.lora \
  --model /path/to/compatible-model \
  --train \
  --data ./data \
  --iters 600 \
  --mask-prompt
```

A quantized starting model selects its QLoRA path. This is useful for training a model to output an answer token such as `A`, `B`, or `C`. 

However, **ordinary completion fine-tuning is not the same training objective or checkpoint architecture as Kev or Clef**. For the actual custom-head approach, use the decision trainer rather than substituting the generic language-model LoRA command.

---

## 8. Hugging Face and other tutorials worth using

| Resource | What it teaches | How I would use it |
|---|---|---|
| **Hugging Face SetFit quickstart** | Few-shot sentence-encoder classification | Establish a cheap fixed-label baseline |
| **Transformers text-classification tutorial** | Tokenization, classification heads, batching, training and evaluation | Learn the standard supervised workflow |
| **PEFT quantization guide** | Quantized backbones and trainable adapters | Understand QLoRA memory and preparation |
| **GLiClass training documentation** | Dynamic-label classification and its training pipeline | Explore a smaller alternative to decoder-based decision models |
| **Together’s “How to train your own Jev for $17”** | Multitask data preparation and supervised answer selection | Study the dataset-building approach, while recognizing the architectural difference |

These resources cover complementary pieces rather than interchangeable recipes. [Hugging Face](https://huggingface.co/docs/setfit/en/quickstart)

### Do not overlook SetFit

When your actual requirement is “route messages into these twelve categories,” a small fixed-label model may be enough.

The current SetFit API uses `SetFitModel`, `Trainer`, and `TrainingArguments`; it trains a sentence representation and classification layer rather than a general-purpose dynamic decision engine. Its quickstart is specifically designed around small labeled datasets. [Hugging Face](https://huggingface.co/docs/setfit/en/quickstart)

My recommendation is to make this your baseline even when you ultimately expect to use Kev. It gives you a useful comparison: **how much additional accuracy or flexibility does the larger decision model actually buy?**

### GLiClass is closer to dynamic-label classification

GLiClass is relevant when candidate labels change but you do not necessarily need a full instruction-following decoder model. Its training documentation includes multiple architectures, label augmentation, and optional LoRA. It is worth testing as an intermediate point between a fixed classifier and a multi-billion-parameter decision model. [Knowledgator](https://docs.knowledgator.com/docs/frameworks/gliclass/training/)

### The Together tutorial is not an exact Kev/Clef reproduction

Together’s example fine-tunes Qwen on decision tasks and then asks it to emit the selected answer letter. Its published inference settings still specify a token-generation budget.

That makes it a useful **supervised answer-selection tutorial**, but not evidence that it reproduces the specialized non-generative heads in Kev or Clef. Its advertised training cost also should not be confused with the continuing cost of the dedicated serving endpoint shown in the tutorial. [Together AI](https://www.together.ai/blog/how-to-train-your-own-jev)

---

## 9. Evaluation, calibration, and deployment

### Accuracy is only the beginning

For a deployment evaluation, I would report:

| Measure | Why it matters |
|---|---|
| Accuracy and macro-F1 | Overall correctness and minority-class performance |
| Per-class precision and recall | Which mistakes the system makes |
| Brier score and negative log-likelihood | Quality of the probability distribution |
| Reliability by confidence bin | Whether confidence corresponds to observed correctness |
| Error rate among automated decisions | Reliability at your selected operating threshold |
| Coverage at that error rate | How much work the model can safely automate |
| Latency and peak memory | Whether the deployment is operationally worthwhile |

Kev’s calibration tooling already reports several of these, including Brier score, calibration error, and coverage under an error constraint. 

### Calibration does not turn uncertainty into a guarantee

Temperature scaling changes probabilities:

$$
p_i=\operatorname{softmax}(z_i/T)
$$

For a single positive temperature, it does not change which candidate has the largest score. It can improve how confidence corresponds to outcomes without improving classification accuracy. Kev’s calibration implementation explicitly treats those as separate properties. 

Choose operating thresholds on held-out data, then measure them on the final test set. A threshold that worked on one department, language, schema, or time period may not transfer unchanged.

### Do not assume `confidence` means the same thing everywhere

This is a significant interoperability issue.

Kev documents a chance-adjusted choice-confidence measure, while Clef’s published answer-conversion code uses the largest option probability. The same numeric threshold therefore need not mean the same thing across the two implementations. Prefer the explicit probability distribution and calibrate the operating rule for the model you deploy.  [Hugging Face](https://huggingface.co/Cloudflare/clef/raw/main/joint_schema_model.py)

### Test more than ordinary examples

My deployment checklist would include long inputs with evidence near the end, missing information, conflicting evidence, new labels, paraphrased policies, option reordering, changed question bundles, and instructions embedded maliciously inside the state.

Also test the **exported model at its actual serving precision**. A result measured before quantization or on a different backend is not automatically the result your application will receive.

For high-impact actions, use the classifier to inform a controlled workflow—not as a replacement for deterministic authorization checks.

### Record licenses and provenance separately

Check the model, datasets, and training/serving code independently. For example, the reviewed Unsloth MLX decision module carries an `AGPL-3.0-only` header, while the project’s package metadata declares Apache-2.0. Do not infer every component’s license from the model card or package summary.  

---

## What I would do in your position

**On an NVIDIA gaming PC:** start with Kev-0.8B and Kev-4B as existing-model baselines. Fine-tune the smallest one that looks promising. Use Unsloth’s quantized decision trainer when memory is tight or when you want to experiment with a fresh Clef-style head.

**On Apple Silicon:** start with Unsloth’s native MLX decision-training path and a 0.8B backbone. Move toward 4B only after the data pipeline and evaluation are working. Kev remains useful as a separately trained model you can serve through MLX.

**For a production workload:** keep a SetFit baseline, an untouched test set, and an explicit “defer to another system or a person” policy. Pick the model that delivers the best measured automation coverage at your acceptable error rate—not the one with the largest parameter count.

The central opportunity is straightforward: **a pretrained backbone, a small decision head, adapter training, good labeled examples, and workload-specific calibration can produce a useful private decision service on retail hardware.** Reproducing the whole proprietary Jev training process is not a prerequisite.
