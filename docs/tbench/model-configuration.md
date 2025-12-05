# Model Configuration

This guide covers how to configure different models for Terminal-Bench overnight runs.

## Supported Models

| Type | Model String | Requirements |
|------|--------------|--------------|
| Claude Code | `claude-code` | Claude Code CLI installed, Claude Max subscription |
| Ollama | `ollama:<model>` | Ollama running locally with model pulled |

## Claude Code (Default)

Claude Code is the default and recommended model for Terminal-Bench runs.

### Requirements

1. **Claude Code CLI**: Install from [claude.ai/code](https://claude.ai/code)
2. **Claude Max subscription**: Required for local authentication
3. **Internet connection**: Claude Code calls Anthropic's API

### Usage

```bash
# Explicit (default)
bun src/cli/tbench-iterate.ts --model claude-code --suite tasks/terminal-bench-2.json

# Implicit (claude-code is default)
bun src/cli/tbench-iterate.ts --suite tasks/terminal-bench-2.json
```

### Verification

```bash
# Check Claude Code is available
which claude

# Test basic functionality
claude --help
```

## Ollama Models

Ollama enables running open-source models locally without API costs.

### Setup

1. **Install Ollama**: Download from [ollama.com](https://ollama.com)

2. **Start Ollama server**:
   ```bash
   ollama serve
   # Default: http://localhost:11434
   ```

3. **Pull a coding model**:
   ```bash
   # Recommended models for coding tasks
   ollama pull codellama:34b
   ollama pull deepseek-coder:33b
   ollama pull qwen2.5-coder:32b
   ```

4. **Verify model is available**:
   ```bash
   curl http://localhost:11434/api/tags | jq '.models[].name'
   ```

### Usage

```bash
# Use CodeLlama 34B
bun src/cli/tbench-iterate.ts \
  --model ollama:codellama:34b \
  --suite tasks/terminal-bench-2.json

# Use DeepSeek Coder
bun src/cli/tbench-iterate.ts \
  --model ollama:deepseek-coder:33b \
  --suite tasks/terminal-bench-2.json

# Custom Ollama endpoint (remote server)
bun src/cli/tbench-iterate.ts \
  --model ollama:codellama:34b \
  --ollama-endpoint http://gpu-server:11434 \
  --suite tasks/terminal-bench-2.json
```

### Model String Format

```
ollama:<model-name>[:<tag>]

Examples:
  ollama:codellama           # Latest codellama
  ollama:codellama:34b       # Specific size
  ollama:codellama:34b-instruct  # Specific variant
  ollama:deepseek-coder:33b
  ollama:qwen2.5-coder:32b
```

### Recommended Models

| Model | Size | Strengths | Considerations |
|-------|------|-----------|----------------|
| `codellama:34b` | 34B | Good coding, tool use | Requires ~20GB VRAM |
| `deepseek-coder:33b` | 33B | Strong reasoning | Requires ~20GB VRAM |
| `qwen2.5-coder:32b` | 32B | Recent, multilingual | Requires ~20GB VRAM |
| `codellama:13b` | 13B | Faster, less VRAM | Lower accuracy |
| `codellama:7b` | 7B | CPU-friendly | Lowest accuracy |

### Hardware Requirements

| Model Size | Min VRAM | Min RAM (CPU) | Notes |
|------------|----------|---------------|-------|
| 7B | 8GB | 16GB | Can run on most systems |
| 13B | 12GB | 32GB | Good laptop GPU |
| 33-34B | 20GB | 64GB | High-end GPU required |

## Model Comparison

### Claude Code vs Ollama

| Aspect | Claude Code | Ollama |
|--------|-------------|--------|
| Cost | Claude Max subscription | Free (hardware costs) |
| Speed | Fast (cloud inference) | Varies by hardware |
| Quality | Best results | Model-dependent |
| Offline | No | Yes |
| Privacy | Data sent to Anthropic | Local only |

### When to Use Each

**Claude Code**:
- Production benchmark runs
- Official leaderboard submissions
- Best-quality results needed

**Ollama**:
- Development and testing
- Cost-sensitive iterations
- Privacy-sensitive code
- Offline environments

## Mixed Mode Validation

Use Claude Code to validate a percentage of Ollama runs:

```bash
# 90% Ollama, 10% Claude validation
bun src/cli/tbench-iterate.ts \
  --model ollama:codellama:34b \
  --claude-validation-rate 0.1 \
  --suite tasks/terminal-bench-2.json \
  --iterations 20
```

This approach:
- Reduces costs while maintaining quality checks
- Identifies model-specific failures
- Builds comparison datasets

## Custom Endpoints

### Remote Ollama Server

```bash
# GPU server on local network
bun src/cli/tbench-iterate.ts \
  --model ollama:codellama:34b \
  --ollama-endpoint http://192.168.1.100:11434 \
  --suite tasks/terminal-bench-2.json
```

### SSH Tunnel for Remote Ollama

```bash
# Set up SSH tunnel
ssh -L 11434:localhost:11434 gpu-server

# Use localhost (tunneled)
bun src/cli/tbench-iterate.ts \
  --model ollama:codellama:34b \
  --suite tasks/terminal-bench-2.json
```

## Troubleshooting

### Ollama Connection Failed

```bash
# Check Ollama is running
curl http://localhost:11434/api/tags

# Start if needed
ollama serve
```

### Model Not Found

```bash
# List available models
ollama list

# Pull missing model
ollama pull codellama:34b

# Verify
curl http://localhost:11434/api/tags | jq '.models[].name'
```

### Out of Memory

```bash
# Use smaller model
ollama:codellama:13b
ollama:codellama:7b

# Or increase swap (not recommended for performance)
```

### Slow Inference

1. **GPU not detected**: Check `nvidia-smi` or `rocm-smi`
2. **Model too large**: Use smaller variant
3. **Background processes**: Free up GPU memory

### Tool Calling Issues

Some Ollama models have limited tool-calling support. If you see many failures:

1. Try a different model (CodeLlama works well)
2. Check model supports function calling
3. Report issues with specific task output

## Environment Variables

```bash
# Override default Ollama endpoint
export OLLAMA_ENDPOINT=http://gpu-server:11434

# Then omit --ollama-endpoint flag
bun src/cli/tbench-iterate.ts --model ollama:codellama:34b ...
```

## Health Check

Before starting a long run, verify model health:

```bash
# Check Claude Code
which claude && echo "Claude Code: OK" || echo "Claude Code: NOT FOUND"

# Check Ollama
curl -s http://localhost:11434/api/tags > /dev/null && echo "Ollama: OK" || echo "Ollama: NOT RUNNING"

# List Ollama models
curl -s http://localhost:11434/api/tags | jq -r '.models[].name'
```
