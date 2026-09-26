---
id: method.huggingface-local-transformer-inference-api
version: 1
kind: method
title: Package a Hugging Face classifier for local API inference
summary: >-
  Download a pretrained sequence classifier and tokenizer once, save both
  together, then load exclusively from that local directory in an HTTP
  service. Applies when turning a Hugging Face text classifier into a
  self-contained local inference endpoint.
tags: [huggingface, transformers, flask, inference, deployment]
applies_when: >-
  A Python HTTP endpoint serves a pretrained Hugging Face
  sequence-classification model and the model should not require network
  access after setup.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - hf-model-inference
  cites:
    - Hugging Face, Transformers documentation, “Auto Classes” and “Models” (loading and saving pretrained models)
    - PyTorch, documentation for `torch.no_grad` and `torch.nn.functional.softmax`
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details
Use `AutoTokenizer.from_pretrained(model_id)` and `AutoModelForSequenceClassification.from_pretrained(model_id)` to acquire compatible artifacts, then call `save_pretrained(directory)` on both. At service startup, load both from that directory rather than the hub identifier; call `model.eval()` before inference. Tokenize with tensor output and an explicit truncation/maximum-length policy. For classification, run under `torch.no_grad()` and apply softmax to logits to obtain class probabilities. Do not assume class-index ordering universally: inspect `model.config.id2label` (and normalize label names) before mapping scores to API fields. Validate request JSON, required fields, types, and empty input before tokenization, and return structured client errors with an appropriate 4xx status. Keep model initialization outside the request handler to avoid repeated loading.

Sources: Hugging Face Transformers documentation, “Auto Classes” and “Models”, sections on loading pretrained models and `save_pretrained`; PyTorch documentation, “Locally disabling gradient computation” (`torch.no_grad`) and `torch.nn.functional.softmax`.

## How to check
Start with network disabled or otherwise verify that the service loads artifacts from the saved directory. For a classifier, assert probabilities are finite, each lies in `[0, 1]`, and their sum is approximately one; verify the label mapping against `model.config.id2label`. Exercise malformed JSON, missing field, wrong field type, and blank text, checking structured 4xx responses. Submit representative inputs from contrasting classes and ensure the service returns valid JSON.
