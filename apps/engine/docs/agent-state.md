Agent state will be saved and restored from a local JSON file.

It should look like this:
```json
{
  "current_repo": {
    "owner": "username",
    "name": "repository-name",
    "branch": "main",
    "current_issue": 123
  },
  "status": "in_progress", // Can be: idle, in_progress, blocked, completed, error
  "plan": [
    {
      "step": 1,
      "description": "Analyze issue requirements",
      "status": "completed"
    },
    {
      "step": 2,
      "description": "Research existing codebase",
      "status": "in_progress"
    }
  ],
  "context": {
    "last_action_timestamp": "2024-03-20T10:30:00Z",
    "related_issues": [121, 122],
    "dependencies": ["issue-124", "PR-45"],
    "files_modified": ["src/main.ts", "tests/main.test.ts"]
  },
  "memory": {
    "conversation_history": [],
    "key_decisions": [],
    "important_findings": []
  },
  "metrics": {
    "steps_completed": 1,
    "total_steps": 5,
    "time_spent": 1800,
    "commits_made": 2
  },
  "error_state": {
    "last_error": null,
    "retry_count": 0,
    "blocked_reason": null
  },
  "configuration": {
    "max_retries": 3,
    "allowed_actions": ["read", "write", "commit", "create_pr"],
    "restricted_files": ["config/*", "secrets/*"],
    "timeout_minutes": 60
  }
}
```
