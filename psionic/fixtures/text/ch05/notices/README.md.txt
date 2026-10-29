---
license: mit
library_name: transformers
base_model: Qwen/Qwen3.5-9B
pipeline_tag: text-classification
language:
- en
tags:
- decision-model
- classification
- calibration
- routing
- guardrails
- vision
- multimodal
- baseten
---

<div align="center">
  <img
    src="https://huggingface.co/baselabs/elephant-1/resolve/main/assets/banner.png"
    alt="Baseten elephant-1 decision model"
    style="width: 100%; max-width: 100%; height: auto; display: inline-block; margin-bottom: 0.5em; margin-top: 0.5em;"
  />
  <div style="display: flex; justify-content: center; gap: 0.5em; margin-bottom: 1em;">
    <a href="https://www.baseten.co/library/elephant-1/"><strong>Try on Baseten</strong></a> •
    <a href="https://labs.baseten.co/articles/mammoth-1-and-elephant-1-frontier-decision-models"><strong>Technical report</strong></a> •
    <a href="https://www.baseten.co/blog/the-best-decision-models-in-the-world-mammoth-1-and-elephant-1/"><strong>Blog</strong></a>
  </div>
</div>

# elephant-1

elephant-1 is a 9B parameter **decision model** post-trained from [Qwen3.5-9B](https://huggingface.co/Qwen/Qwen3.5-9B) by Base Labs at Baseten.
Provide the model a state (text, optionally with images) and a set of questions and it will return a calibrated probability for every option,
answering all questions from **one shared-prefix prefill**. On tested benchmarks:

- **Best model under 15B on the Jev Decision Index 0.4**: 62.65, tied #8 of 208 models.
- **65.2 on a replica of the Artificial Analysis Decision Index**: level with d1-ultra (65).

For tasks that need broader general knowledge, see the 27B sibling [mammoth-1](https://huggingface.co/baselabs/mammoth-1).

## 🗒️ Model Details

| Model | Parameters | Description |
|---|---|---|
| [Qwen3.5-9B](https://huggingface.co/Qwen/Qwen3.5-9B) | 9B | General-purpose base model |
| **[elephant-1](https://huggingface.co/baselabs/elephant-1)** | 9B | Post-trained for single-pass, calibrated decisions |
| [mammoth-1](https://huggingface.co/baselabs/mammoth-1) | 27B | Larger sibling, highest accuracy |

- **Inputs**: text and images
- **Training**: full fine-tune of the language model on text and image decisions; the vision encoder is frozen
- **Attention**: block-bidirectional mask on full-attention layers.
- **Max sequence length**: 16,384 tokens

## 🏃 How to use

### Baseten Model APIs

elephant-1 is free for one week on [Baseten Model APIs](https://www.baseten.co/library/elephant-1/), with zero data retention on by default. The free week ends 10/27/26 at 11:59 PM PT. The endpoint accepts text and image inputs.


### Self-hosted

`setup.sh` installs the pinned serving runtime (OneJev + `serve.patch`). It needs one CUDA GPU and the `pytorch/pytorch:2.8.0-cuda12.8-cudnn9-devel` image.

```bash
hf download baselabs/elephant-1 --local-dir elephant-1 && bash elephant-1/setup.sh
```

```python
from qev.engine import DecisionEngine
from qev.schema import SystemOneRequest

engine = DecisionEngine("baselabs/elephant-1", device="cuda:0")

questions = {
    "refund": {"type": "noul", "instructions": "Is the customer asking for a refund?"},
    "team": {
        "type": "choice",
        "instructions": "Which team should handle this?",
        "criteria": {"billing": "Charges, refunds, invoices",
                     "technical": "App or site faults",
                     "fraud": "Suspected unauthorised use"},
    },
    "urgency": {
        "type": "score",
        "instructions": "How urgent is this?",
        "criteria": ["Can wait", "Today", "Blocking the customer now"],
    },
}
resp, _ = engine.decide(SystemOneRequest(
    state="I was charged twice this month, please refund one of them.", questions=questions))
print(resp.answers)
```

| `type` | `criteria` | answer fields |
|---|---|---|
| `noul`: yes or no | optional | `noul`: P(yes) |
| `choice`: one of named options | `{name: description}` | `choice`, `probabilities` |
| `score`: ordered levels | list of levels, lowest first | `score`, `probabilities` |

Requests longer than the trained length limits are refused, not truncated.

## 📊 Performance

### Jev Decision Index 0.4

| Model | Decision Index |
|---|---:|
| mammoth-1 | 71.24 |
| Perplexity Decider v1.2 | 69.42 |
| **elephant-1** | **62.65** |
| Jev 1.14 | 62.2 |
| Cloudflare Clef | 55.7 |

On the public split, elephant-1 narrowly trails mammoth-1 in every area and beats Jev 1.14 in all five (knowledge & reasoning, language, tools & automation, retrieval & classification, arts & human taste).

### Artificial Analysis Decision Index (our replica)

| Model | Score |
|---|---:|
| mammoth-1 | 65.9 |
| **elephant-1** | **65.2** |
| d1-ultra | 65 |

## 🔧 Training (all details in the technical report)

- **Data**: 6.21M decision examples (Mix 8). 58.3% core decision skills, 16.1% safety and abstention, 16.1% judging and expert domains, 9.5% public decision data.
- **Objective**: cross-entropy plus a Brier calibration term (λ = 1), with a trained "I don't know" option.
- **Recipe**: 1 epoch (5,546 steps), peak LR 3.5e-6, AdamW (β 0.9/0.95, no weight decay), 3% warm-up, cosine decay, bf16.
- **Hardware**: 16 nodes × 8 H100 (128 GPUs).

## Citation

```bibtex
@article{baseten2026mammoth,
  author  = {Base Labs, Baseten},
  title   = {mammoth-1 and elephant-1: Frontier Decision Models},
  journal = {Base Labs},
  year    = {2026},
  note    = {https://labs.baseten.co/articles/mammoth-1-and-elephant-1-frontier-decision-models},
}
```
