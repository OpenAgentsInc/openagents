# `suite_manifest_view`

```json
{
  "schema": "openagents.gym.suite_manifest_view.v1",
  "suite_id": "tb2-quick",
  "suite_digest": "suite-manifest:0c0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f",
  "tier": "score",
  "description": "Two quick Terminal-Bench 2.0 tasks",
  "task_count": 2,
  "source_path": "bench/suites/tb2-quick.suite.json",
  "tasks": [
    {
      "id": "regex-log",
      "task_digest": "task:a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1",
      "environment_available": true,
      "rationale": "near-zero tool surface",
      "pin": {
        "kind": "harbor_registry",
        "dataset": "terminal-bench@2.0",
        "git_url": "https://github.com/laude-institute/terminal-bench-2.git",
        "commit": "69671fbaac6d67a7ef0dfec016cc38a64ef7a77c",
        "path": "regex-log"
      }
    },
    {
      "id": "openssl-selfsigned-cert",
      "task_digest": "task:b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2",
      "environment_available": true,
      "rationale": "fully specified checklist",
      "pin": {
        "kind": "harbor_registry",
        "dataset": "terminal-bench@2.0",
        "git_url": "https://github.com/laude-institute/terminal-bench-2.git",
        "commit": "69671fbaac6d67a7ef0dfec016cc38a64ef7a77c",
        "path": "openssl-selfsigned-cert"
      }
    }
  ]
}
```
