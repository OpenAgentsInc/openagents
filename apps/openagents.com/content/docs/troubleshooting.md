---
title: Troubleshooting
date: 2024-12-17
summary: Common issues and solutions for OpenAgents development
category: guide
order: 4
---

# Troubleshooting

Common issues and solutions when working with OpenAgents. If you can't find your issue here, please [open a GitHub issue](https://github.com/OpenAgentsInc/openagents/issues).

## Installation Issues

### `Module not found: @openagentsinc/sdk`

**Symptoms**: Import errors when trying to use the SDK

**Solution**:
```bash
# Clear package manager cache
pnpm store prune
# or
npm cache clean --force

# Reinstall dependencies
rm -rf node_modules package-lock.json
pnpm install
```

### TypeScript compilation errors

**Symptoms**: Type errors when importing SDK functions

**Solution**: Ensure you're using Node.js 18+ with proper TypeScript configuration:

```json
// tsconfig.json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "node",
    "allowSyntheticDefaultImports": true,
    "esModuleInterop": true,
    "strict": true
  }
}
```

## Ollama Connection Issues

### `Ollama is not available` errors

**Symptoms**: SDK functions fail with Ollama connection errors

**Common Causes**:
1. Ollama not installed
2. Ollama not running
3. Ollama running on wrong port
4. Firewall blocking connection

**Solutions**:

**Install Ollama**:
```bash
# macOS
brew install ollama

# Linux
curl -fsSL https://ollama.com/install.sh | sh

# Windows
# Download from https://ollama.com/download
```

**Start Ollama**:
```bash
# Start Ollama service
ollama serve

# Pull a model
ollama pull llama3.2
```

**Check Connection**:
```bash
# Test if Ollama is responding
curl http://localhost:11434/api/tags

# Should return JSON with available models
```

**Alternative Port Configuration**:
```typescript
// If Ollama runs on different port
const customConfig = {
  ollamaUrl: "http://localhost:11435" // Custom port
}
```

### Models not loading

**Symptoms**: `listModels()` returns empty array

**Solution**:
```bash
# List available models online
ollama list

# Pull popular models
ollama pull llama3.2
ollama pull mistral
ollama pull codellama

# Verify models loaded
curl http://localhost:11434/api/tags
```

## Agent Creation Issues

### `Agent creation failed` errors

**Symptoms**: `Agent.create()` throws errors

**Common Causes**:
1. Invalid configuration
2. Network connectivity issues
3. Insufficient system resources

**Solutions**:

**Validate Configuration**:
```typescript
// Minimal valid configuration
const agent = Agent.create({
  name: "Test Agent", // Required: non-empty string
  capabilities: ["test"] // Optional but recommended
})

// Avoid these common mistakes
const badAgent = Agent.create({
  name: "", // ❌ Empty name
  pricing: {
    per_request: -100 // ❌ Negative pricing
  }
})
```

**Debug Agent Creation**:
```typescript
try {
  const agent = Agent.create({ name: "Debug Agent" })
  console.log("✅ Agent created successfully")
} catch (error) {
  console.error("❌ Agent creation failed:", error.message)
  
  if (error instanceof ValidationError) {
    console.error("Configuration error:", error.details)
  }
}
```

### Mnemonic generation fails

**Symptoms**: `generateMnemonic()` or `createFromMnemonic()` errors

**Solution**:
```typescript
// Test mnemonic generation
try {
  const mnemonic = await Agent.generateMnemonic()
  console.log("✅ Mnemonic generated:", mnemonic)
  
  const agent = await Agent.createFromMnemonic(mnemonic)
  console.log("✅ Agent created from mnemonic")
} catch (error) {
  console.error("❌ Mnemonic error:", error.message)
  
  // Check system entropy
  if (error.message.includes("entropy")) {
    console.log("Try running in environment with better randomness")
  }
}
```

## Lightning Network Issues

### Lightning invoice creation fails

**Symptoms**: `createLightningInvoice()` throws errors

**Common Causes**:
1. No Lightning node connected
2. Insufficient channel capacity
3. Invalid invoice parameters

**Solutions**:

**Validate Invoice Parameters**:
```typescript
// Correct invoice creation
const invoice = Agent.createLightningInvoice(agent, {
  amount: 1000,        // ✅ Positive integer (sats)
  memo: "Test payment" // ✅ Non-empty memo
})

// Avoid these mistakes
const badInvoice = Agent.createLightningInvoice(agent, {
  amount: 0,    // ❌ Zero amount
  memo: ""      // ❌ Empty memo
})
```

**Test Lightning Connection**:
```bash
# If using LND
lncli getinfo

# If using CLN
lightning-cli getinfo

# Check available balance
lncli walletbalance
```

### Payment timeout issues

**Symptoms**: Invoices expire before payment

**Solution**:
```typescript
// Create invoice with longer expiry
const invoice = Agent.createLightningInvoice(agent, {
  amount: 10000,
  memo: "Extended payment",
  // Add custom expiry if supported
  expiry: 3600 // 1 hour instead of default 15 minutes
})

// Monitor invoice status
const checkPayment = setInterval(async () => {
  const status = await invoice.getStatus()
  if (status === "paid") {
    console.log("✅ Payment received")
    clearInterval(checkPayment)
  }
}, 5000) // Check every 5 seconds
```

## Inference Issues

### Model not responding

**Symptoms**: `Inference.infer()` hangs or times out

**Common Causes**:
1. Model not loaded in Ollama
2. Request too complex for model
3. System resource constraints

**Solutions**:

**Check Model Availability**:
```typescript
// List available models first
const models = await Inference.listModels()
console.log("Available models:", models.map(m => m.id))

// Use available model
const response = await Inference.infer({
  system: "You are helpful",
  messages: [{ role: "user", content: "Hello" }],
  model: models[0]?.id || "llama3.2", // Use first available
  max_tokens: 100
})
```

**Reduce Request Complexity**:
```typescript
// If inference fails, try simpler request
const simpleRequest = {
  system: "Be brief",
  messages: [{ role: "user", content: "Hi" }],
  max_tokens: 50,    // Reduce token limit
  temperature: 0.1   // Reduce randomness
}

const response = await Inference.infer(simpleRequest)
```

**Add Timeout Handling**:
```typescript
// Add timeout to prevent hanging
const timeoutPromise = new Promise((_, reject) => {
  setTimeout(() => reject(new Error("Timeout")), 30000) // 30 second timeout
})

try {
  const response = await Promise.race([
    Inference.infer(request),
    timeoutPromise
  ])
} catch (error) {
  if (error.message === "Timeout") {
    console.error("Inference timed out, try simpler request")
  }
}
```

### Streaming interruptions

**Symptoms**: `inferStream()` stops mid-response

**Solution**:
```typescript
// Robust streaming with error recovery
async function robustStreaming(request) {
  let retries = 3
  
  while (retries > 0) {
    try {
      let content = ""
      
      for await (const chunk of Inference.inferStream(request)) {
        content += chunk.content
        process.stdout.write(chunk.content)
        
        // Check for completion
        if (chunk.finish_reason === "stop") {
          return content
        }
      }
    } catch (error) {
      console.error(`Streaming error: ${error.message}`)
      retries--
      
      if (retries > 0) {
        console.log(`Retrying... (${retries} attempts left)`)
        await new Promise(resolve => setTimeout(resolve, 1000))
      }
    }
  }
  
  throw new Error("Streaming failed after retries")
}
```

## Performance Issues

### Slow response times

**Symptoms**: Inference takes >30 seconds

**Solutions**:

**Use Smaller Models**:
```typescript
// Fast models for quick responses
const fastModels = [
  "llama3.2-1b",    // Fastest, basic quality
  "llama3.2-3b",    // Good balance
  "qwen2.5-1.5b"    // Very fast
]

const response = await Inference.infer({
  system: "Be concise",
  messages: [{ role: "user", content: query }],
  model: "llama3.2-1b", // Use fastest model
  max_tokens: 100       // Limit response length
})
```

**Optimize System Resources**:
```bash
# Check system resources
top
htop
nvidia-smi # If using GPU

# Increase Ollama memory limit
export OLLAMA_MAX_LOADED_MODELS=1
export OLLAMA_NUM_PARALLEL=1
```

**Implement Caching**:
```typescript
// Simple response cache
const responseCache = new Map()

async function cachedInference(request) {
  const key = JSON.stringify(request)
  
  if (responseCache.has(key)) {
    return responseCache.get(key)
  }
  
  const response = await Inference.infer(request)
  responseCache.set(key, response)
  
  return response
}
```

### High memory usage

**Symptoms**: System becomes unresponsive, out of memory errors

**Solutions**:

**Limit Concurrent Requests**:
```typescript
// Request queue to prevent overload
class RequestQueue {
  constructor(maxConcurrent = 2) {
    this.maxConcurrent = maxConcurrent
    this.running = 0
    this.queue = []
  }
  
  async add(requestFn) {
    return new Promise((resolve, reject) => {
      this.queue.push({ requestFn, resolve, reject })
      this.process()
    })
  }
  
  async process() {
    if (this.running >= this.maxConcurrent || this.queue.length === 0) {
      return
    }
    
    this.running++
    const { requestFn, resolve, reject } = this.queue.shift()
    
    try {
      const result = await requestFn()
      resolve(result)
    } catch (error) {
      reject(error)
    } finally {
      this.running--
      this.process() // Process next item
    }
  }
}

const queue = new RequestQueue(2) // Max 2 concurrent requests
```

**Configure Ollama Memory**:
```bash
# Limit Ollama memory usage
export OLLAMA_MAX_LOADED_MODELS=1
export OLLAMA_HOST=0.0.0.0:11434

# Start with memory limits
ollama serve
```

## Development Issues

### Hot reload not working

**Symptoms**: Changes not reflected during development

**Solution**:
```bash
# Clear build cache
rm -rf .next
rm -rf dist
rm -rf build

# Restart development server
pnpm dev

# If using Bun, ensure hot reload enabled
bun run --hot src/index.ts
```

### TypeScript errors in development

**Symptoms**: Type checking fails unexpectedly

**Solution**:
```bash
# Update TypeScript and dependencies
pnpm update typescript @types/node

# Rebuild project with clean slate
pnpm clean
pnpm build

# Check TypeScript configuration
npx tsc --showConfig
```

## Getting Help

### Enable Debug Logging

```typescript
// Enable verbose logging
process.env.DEBUG = "openagents:*"

// Or specific modules
process.env.DEBUG = "openagents:inference,openagents:agent"
```

### Collect System Information

```bash
# System info for bug reports
echo "Node.js: $(node --version)"
echo "npm: $(npm --version)"
echo "OS: $(uname -a)"
echo "Ollama: $(ollama --version)"

# Package versions
pnpm list @openagentsinc/sdk
```

### Create Minimal Reproduction

```typescript
// Minimal example for bug reports
import { Agent, Inference } from '@openagentsinc/sdk'

async function reproduce() {
  try {
    // Your minimal failing case here
    const agent = Agent.create({ name: "Bug Report" })
    console.log("Agent created:", agent.id)
  } catch (error) {
    console.error("Error:", error.message)
    console.error("Stack:", error.stack)
  }
}

reproduce()
```

### Community Resources

- **GitHub Issues**: [Report bugs](https://github.com/OpenAgentsInc/openagents/issues)
- **Discord**: Join our community chat
- **Documentation**: [Full API reference](./api-reference)
- **Examples**: [GitHub repository examples](https://github.com/OpenAgentsInc/openagents/tree/main/examples)

---

*Can't find your issue? [Open a GitHub issue](https://github.com/OpenAgentsInc/openagents/issues/new) with details about your problem.* 🐛