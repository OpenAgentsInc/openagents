# Ollama CORS Configuration

To allow the OpenAgents SDK to connect to Ollama from web applications, you need to configure CORS.

## Option 1: Set CORS Origins (Recommended)

When starting Ollama, set the allowed origins:

```bash
OLLAMA_ORIGINS="*" ollama serve
```

Or for specific origins:
```bash
OLLAMA_ORIGINS="http://localhost:*" ollama serve
```

## Option 2: Use Environment Variable

Set the environment variable permanently:

```bash
# On macOS/Linux
export OLLAMA_ORIGINS="*"

# On Windows
set OLLAMA_ORIGINS=*
```

## Option 3: Ollama Configuration File

Create or edit `~/.ollama/config.json`:

```json
{
  "origins": ["http://localhost:*"]
}
```

## Verify Configuration

After restarting Ollama with CORS enabled, you should see proper CORS headers in the response:

```bash
curl -I http://localhost:11434/api/tags
```

Should include:
```
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: GET, POST, OPTIONS
Access-Control-Allow-Headers: Content-Type, Authorization
```

## Troubleshooting

1. **404 Errors**: Make sure the model exists. List available models:
   ```bash
   curl http://localhost:11434/api/tags
   ```

2. **CORS Errors**: Restart Ollama with proper ORIGINS set

3. **Connection Refused**: Ensure Ollama is running:
   ```bash
   ollama serve
   ```