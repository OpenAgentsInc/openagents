# TB2 End-to-End Verification Test

**Date:** 2024-12-11 13:49
**Status:** Complete - All Infrastructure Verified Working

## Goal

Programmatically verify the entire TB2 Docker integration works end-to-end:
1. Claude Code authentication
2. Claude writing files to /app
3. Docker container execution
4. TB2 verification tests
5. reward.txt result parsing

## Test Sequence

### Step 1: Build Docker Image Locally

TB2 images aren't published to Docker Hub - they must be built locally from each task's Dockerfile.

```bash
cd ~/code/terminal-bench-2/regex-log/environment
docker build -t alexgshaw/regex-log:20251031 .
```

The Dockerfile is minimal (Ubuntu 24.04 + WORKDIR /app):
```dockerfile
FROM ubuntu:24.04
WORKDIR /app
```

### Step 2: Test Claude Code Authentication

First, verified Claude CLI auth works with a simple task:

```bash
cd /tmp && mkdir -p mechacoder-test && cd mechacoder-test
claude --verbose -p "Write 'hello' to /tmp/mechacoder-test/test.txt" \
  --max-turns 3 --output-format stream-json
```

**Result:** Auth working, but permission denied (expected in default mode).

Output showed:
```json
{"type":"system","subtype":"init","model":"claude-opus-4-5-20251101","permissionMode":"default",...}
{"type":"result","subtype":"success","total_cost_usd":0.14380675,...}
```

### Step 3: Test with Auto-Permissions

Re-ran with `--dangerously-skip-permissions`:

```bash
claude --verbose --dangerously-skip-permissions \
  -p "Write 'hello' to /tmp/mechacoder-test/test.txt" \
  --max-turns 3 --output-format stream-json
```

**Result:** Success! File created, cost $0.034.

```bash
$ cat /tmp/mechacoder-test/test.txt
hello
```

### Step 4: Run Actual regex-log Task

Ran Claude Sonnet on the regex-log task (Opus was too slow for testing):

```bash
mkdir -p /tmp/regex-log-test/app && cd /tmp/regex-log-test/app
claude --verbose --dangerously-skip-permissions \
  --model claude-sonnet-4-20250514 \
  -p "Write a regex that matches YYYY-MM-DD dates in lines containing IPv4 addresses. Save to /tmp/regex-log-test/app/regex.txt" \
  --max-turns 5 --output-format stream-json
```

**Result:** Claude wrote regex in ~27 seconds, cost $0.11.

Generated regex:
```
(?=.*\b(?:[0-9]{1,3}\.){3}[0-9]{1,3}\b)\b\d{4}-\d{2}-\d{2}\b
```

### Step 5: Run TB2 Docker Verification

Ran the TB2 test.sh in Docker with the generated regex:

```bash
mkdir -p /tmp/regex-log-test/logs/verifier
docker run --rm \
  -v /tmp/regex-log-test/app:/app \
  -v /tmp/regex-log-test/logs:/logs \
  -v /home/christopherdavid/code/terminal-bench-2/regex-log/tests:/tests:ro \
  -w /app \
  alexgshaw/regex-log:20251031 \
  bash /tests/test.sh
```

**Result:** Test executed successfully, regex failed (expected for single attempt).

### Step 6: Verification Results

```bash
$ cat /tmp/regex-log-test/logs/verifier/reward.txt
0
```

Test output showed the regex matched 4/9 expected dates:
```
Expected: ['2025-01-09', '2024-11-01', '2020-01-01', '2019-06-15', '2018-06-06', '2017-03-03', '2016-12-31', '2024-11-11', '1999-03-05']
Got:      ['2025-01-09', '2019-06-15', '2018-05-05', '2024-11-11']
```

The regex failed because:
- It doesn't pick "last date" when multiple dates exist on a line
- It doesn't validate date ranges (months 01-12, days 01-31)
- It doesn't validate IP address ranges (0-255 per octet)

This is expected - regex-log is a hard task that requires iteration to solve.

## Summary

| Component | Status | Details |
|-----------|--------|---------|
| Claude Code CLI | ✓ Working | Auth via ANTHROPIC_API_KEY |
| stream-json output | ✓ Working | Proper JSON events streamed |
| --dangerously-skip-permissions | ✓ Working | Required for automated runs |
| File writing to /app | ✓ Working | regex.txt created |
| Docker image | ✓ Built locally | alexgshaw/regex-log:20251031 |
| Docker volume mounts | ✓ Working | /app, /logs, /tests mounted |
| TB2 test.sh | ✓ Runs | Installs uv, runs pytest |
| pytest execution | ✓ Working | Tests run, assertions checked |
| reward.txt | ✓ Written | 0 = fail, 1 = pass |
| Cost tracking | ✓ Working | $0.11 for Sonnet run |

## Key Findings

### 1. TB2 Images Must Be Built Locally

TB2 task images (e.g., `alexgshaw/regex-log:20251031`) are NOT on Docker Hub. They must be built from each task's `environment/Dockerfile`:

```bash
cd ~/code/terminal-bench-2/<task>/environment
docker build -t <image-name-from-task.toml> .
```

### 2. Harbor Installs Claude Code at Runtime

Harbor's approach installs Claude Code inside the container using:
```bash
apt-get install -y curl
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.2/install.sh | bash
source "$HOME/.nvm/nvm.sh"
nvm install 22
npm install -g @anthropic-ai/claude-code@latest
```

Our implementation could either:
- Install Claude Code in a base image (faster startup)
- Mount host Claude installation (complex)
- Install at runtime like Harbor (slower but compatible)

### 3. Opus is Slow for Testing

Claude Opus 4.5 took >2 minutes just to start thinking on the regex task. Sonnet completed in ~27 seconds. For iterative testing, use Sonnet. For production runs, Opus may be worth the wait.

### 4. Permission Mode Required

For automated runs, `--dangerously-skip-permissions` is required. The Docker runner should use this flag.

## Test Artifacts

```
/tmp/regex-log-test/
├── app/
│   └── regex.txt              # Generated regex
├── logs/
│   └── verifier/
│       └── reward.txt         # "0" (test failed)
└── output.txt                 # Claude CLI output
```

## Next Steps

1. **Pre-build base image with Claude Code** - Faster container startup
2. **Add --dangerously-skip-permissions** - Required for automated runs
3. **Run full regex-log solve** - Multiple iterations to pass all tests
4. **Test other TB2 tasks** - Verify generalization

## Related Files

- Integration tests: `crates/gym/tests/tb2_integration.rs`
- Docker runner: `crates/gym/src/mechacoder/docker_runner.rs`
- Previous log: `docs/logs/20251211/1329-mechacoder-tb2-docker-integration.md`
