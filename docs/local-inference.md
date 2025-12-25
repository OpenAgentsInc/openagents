# Running GPT-OSS Locally

This guide covers the fastest way to run GPT-OSS models on a workstation with an NVIDIA GPU.

## System Specs

- **CPU**: Intel Core i7-14700K (28 threads)
- **GPU**: NVIDIA GeForce RTX 4080 (16GB VRAM)
- **RAM**: 128GB DDR5
- **OS**: Arch Linux

## Performance Summary

| Model | Backend | Prompt (t/s) | Generation (t/s) |
|-------|---------|--------------|------------------|
| gpt-oss-20b | llama.cpp (GPU) | 1276.7 | 104.0 |
| gpt-oss-120b | llama.cpp (GPU) | 37.4 | 15.9 |
| gpt-oss-120b | llama.cpp (CPU) | 44.1 | 9.3 |
| gpt-oss-120b | Ollama | ~2-3 | ~2-3 |

**Recommended**: Use llama.cpp with GPU acceleration. It's 5-10x faster than Ollama.

## Quick Start

### 1. Get the GGUF Models

Download MXFP4-quantized GGUFs from Hugging Face:

```bash
# 20b model (~12GB)
huggingface-cli download openai/gpt-oss-20b-GGUF gpt-oss-20b-mxfp4.gguf --local-dir ~/models/gpt-oss

# 120b model (~63GB)
huggingface-cli download openai/gpt-oss-120b-GGUF gpt-oss-120b-mxfp4.gguf --local-dir ~/models/gpt-oss
```

### 2. Build llama.cpp with CUDA

```bash
cd ~/code/llama.cpp
cmake -B build -DGGML_CUDA=ON
cmake --build build --config Release -j
```

### 3. Run Inference

```bash
# 20b - fits entirely in GPU VRAM, blazing fast
./llama-cli -m ~/models/gpt-oss/gpt-oss-20b-mxfp4.gguf -p "Your prompt here" -n 100 -t 28

# 120b - uses GPU + CPU offloading automatically
./llama-cli -m ~/models/gpt-oss/gpt-oss-120b-mxfp4.gguf -p "Your prompt here" -n 100 -t 28
```

## Model Details

### gpt-oss-20b
- **Parameters**: 21B total, 3.6B active (MoE)
- **GGUF Size**: 12GB (MXFP4 quantized)
- **Memory**: Fits in 16GB VRAM
- **Best for**: Fast iteration, development, lower latency tasks

### gpt-oss-120b
- **Parameters**: 117B total, 5.1B active (MoE)
- **GGUF Size**: 63GB (MXFP4 quantized)
- **Memory**: Uses GPU + CPU RAM together (~7GB VRAM + ~60GB RAM)
- **Best for**: Complex reasoning, production-quality outputs

## Alternative: Ollama (Slower)

If you prefer simplicity over speed:

```bash
# Already have these downloaded
ollama run gpt-oss:20b
ollama run gpt-oss:120b
```

Ollama wraps llama.cpp but adds overhead. Direct llama.cpp is 5-10x faster.

## Tips

1. **Use GPU when available** - Even partial GPU offloading (120b) gives significant speedups
2. **Match threads to P-cores** - On i7-14700K, use `-t 28` for all threads
3. **Skip Ollama for speed** - Direct llama.cpp is always faster
4. **20b for development** - 104 t/s generation is near-instant for most tasks

## CPU-Only Mode

If you need to disable GPU (not recommended):

```bash
CUDA_VISIBLE_DEVICES="" ./llama-cli -m ~/models/gpt-oss/gpt-oss-120b-mxfp4.gguf -p "prompt" -t 28
```

This drops generation from 15.9 t/s to 9.3 t/s on 120b.

## OpenAgents local-infer runner

If you already have a local GPT-OSS server or the Apple Foundation Models bridge running, use the unified runner:

```bash
scripts/local-infer.sh --backend gpt-oss --url http://localhost:8000 "Hello"
scripts/local-infer.sh --backend fm-bridge --url http://localhost:3030 --tools "Summarize this repo"
```

Use `--tools` to enable local tool calls (browser, python, apply_patch, ui_pane). You can also set `FM_BRIDGE_URL` instead of passing `--url` for the FM bridge.
