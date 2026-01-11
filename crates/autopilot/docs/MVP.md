# Autopilot v0.1

A local-first autonomous coding agent.

## User experience

### 1. Install

User installs Autopilot via either:

1. Shell script from openagents.com
2. Building from source via our GitHub repo (requires Rust toolchain)

### 2. Run

In your working directory, run:

```bash
autopilot
```

This opens the Autopilot desktop app.

### 3. Connect Codex

This version of Autopilot requires a Codex subscription.

Future versions will support other agents and API keys.

### 4. Prompt

Enter your prompt and hit enter. This begins an Autopilot session.


## Concepts

### One Conversation Per Project

Conversation threads are separated only by project, based on its working directory. Each working directory has its own long-running conversation.

### Instant Message Processing 

You can add new prompts anytime. They are known to Autopilot immediately, not queued.

### Continuous Learning

Autopilot responses use DSPy signatures and optimizations. Read more about our DSPy integration [here](https://github.com/OpenAgentsInc/openagents/blob/main/docs/dspy/README.md).


