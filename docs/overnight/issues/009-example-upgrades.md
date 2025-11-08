# Issue #009: Create Example Upgrade Manifests

**Component**: Upgrade Examples
**Priority**: P2 (Medium)
**Estimated Effort**: 1-2 days
**Dependencies**: #006 (UpgradeExecutor)
**Assignee**: TBD

---

## Overview

Create 2-3 example upgrade manifests demonstrating overnight orchestration: nightly-refactor.json, feature-worker.json, test-generator.json.

**Location**: `private/overnight/examples/`

---

## Requirements

1. **nightly-refactor.json**: Code quality improvements every 30 min, 1am-5am
2. **feature-worker.json**: Implement features from backlog
3. **test-generator.json**: Generate tests for uncovered code

Each manifest should demonstrate:
- Cron scheduling with time windows
- Constraints (plugged_in, wifi_only, etc.)
- Full pipeline (session.analyze → orchestrate.decide → agent.execute → pr.create)
- Proper permissions
- Pricing/revenue split (future-ready)

---

## Example: nightly-refactor.json

```json
{
  "$schema": "https://openagents.com/schemas/upgrade-manifest-v1.json",
  "id": "nightly-refactor-v1",
  "version": "1.0.0",
  "title": "Nightly Code Refactoring Agent",
  "description": "Autonomous overnight refactoring with FM-driven decisions",

  "author": {
    "name": "OpenAgents",
    "npub": "npub1...",
    "url": "https://github.com/OpenAgentsInc/openagents"
  },

  "license": "MIT",
  "categories": ["development", "code-quality"],
  "tags": ["refactoring", "automation", "overnight"],

  "capabilities": {
    "platforms": ["macos"],
    "min_macos_version": "26.0",
    "backends": ["foundation_models"],
    "required_tools": ["git", "gh", "swift"]
  },

  "permissions": {
    "filesystem": {
      "read": ["$WORKSPACE/**"],
      "write": ["$WORKSPACE/**"],
      "exclude": [".env", "*.key", "credentials.json", ".git/**"]
    },
    "network": {
      "allowed_domains": ["github.com", "api.github.com"],
      "ports": []
    },
    "tools": {
      "allowed": ["git", "gh", "xcodebuild", "swift"],
      "denied": ["rm", "curl", "wget"]
    }
  },

  "schedule": {
    "type": "cron",
    "expression": "*/30 1-5 * * *",
    "timezone": "America/Los_Angeles",
    "window": {
      "start": "01:00",
      "end": "05:00"
    },
    "constraints": {
      "plugged_in": true,
      "wifi_only": true,
      "cpu_max_percentage": 80,
      "respect_dnd": false,
      "suspend_if_active": true
    },
    "jitter": 300,
    "on_missed": "run_once_at_next_opportunity"
  },

  "triggers": [],

  "pipeline": [
    {
      "op": "session.analyze",
      "params": {
        "providers": ["claude-code", "codex"],
        "topK": 20,
        "since": "7d"
      },
      "output_var": "session_insights"
    },
    {
      "op": "repo.status",
      "params": {
        "working_dir": "$WORKSPACE"
      },
      "output_var": "repo_status"
    },
    {
      "op": "orchestrate.decide",
      "backend": "foundation_models",
      "params": {
        "context": {
          "session_insights": "{session_insights}",
          "repo_status": "{repo_status}"
        },
        "available_agents": ["claude-code", "codex"],
        "time_budget": 1800
      },
      "output_var": "decision"
    },
    {
      "op": "agent.execute",
      "params": {
        "agent": "{decision.agent}",
        "task": "{decision.task}",
        "working_dir": "$WORKSPACE",
        "max_duration": 1800
      },
      "output_var": "session_result"
    },
    {
      "op": "pr.create",
      "params": {
        "branch_prefix": "agent/",
        "base_branch": "main",
        "title": "{decision.task}",
        "body_template": "## Autonomous Agent Work\n\n**Task**: {decision.task}\n\n**Agent**: {session_result.agent}\n\n**Rationale**: {decision.rationale}\n\n**Session**: {session_result.session_id}\n\n**Duration**: {session_result.duration}\n\n---\n\nGenerated with [OpenAgents](https://github.com/OpenAgentsInc/openagents)",
        "auto_push": true,
        "draft": false
      },
      "output_var": "pr_number"
    }
  ],

  "pricing": {
    "model": "free",
    "amount_sats": 0
  },

  "policy": {
    "aup_compliance": true,
    "data_retention": "30d",
    "telemetry_level": "aggregate"
  },

  "signing": null
}
```

---

## Example: test-generator.json

```json
{
  "id": "test-generator-v1",
  "title": "Automated Test Generation",
  "description": "Generate comprehensive tests for low-coverage areas",

  "schedule": {
    "expression": "0 3 * * *",
    "window": {"start": "02:00", "end": "05:00"}
  },

  "pipeline": [
    {
      "op": "repo.coverage",
      "output_var": "coverage"
    },
    {
      "op": "orchestrate.decide",
      "params": {
        "hint": "Generate tests for files with <70% coverage",
        "preferred_agent": "codex"
      },
      "output_var": "decision"
    },
    {
      "op": "agent.execute",
      "params": {
        "agent": "codex",
        "task": "{decision.task}"
      },
      "output_var": "session_result"
    },
    {
      "op": "pr.create",
      "params": {
        "title": "Add tests for {decision.target_file}",
        "draft": false
      }
    }
  ]
}
```

---

## Testing

1. Validate all manifests parse correctly
2. Execute with UpgradeExecutor
3. Verify all ops run successfully

---

## Acceptance Criteria

- [ ] 3 example manifests created
- [ ] All validate successfully
- [ ] Can be executed end-to-end
- [ ] Well-documented with comments

---

## References

- docs/compute/issues/upgrades.md
