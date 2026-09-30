# Introducing Coder

Coder is an all-in-one coding agent. It runs on your computer, works across your repositories, and drives tasks from the terminal or the web console.

## Built for developers

Coder is designed around one principle: direct, transparent interaction with your code.

- **Terminal native**: Install with one command and start working in your repository.
- **Shared core**: The desktop app, terminal, and web console share one rendering pipeline and one component library.
- **MicroVM isolation**: Every cloud execution runs in an isolated runner with verification gates.

## Getting started

Install Coder Terminal on macOS and Linux with `curl`:

```sh
curl -fsSL https://openagents.com/releases/install-terminal.sh | sh
```

On Windows, run in PowerShell:

```powershell
irm https://openagents.com/releases/install-terminal.ps1 | iex
```

## How Coder works

When you give Coder a directive, it inspects your repository, plans modifications, executes tools, and runs tests through a structured verification loop:

| Stage | What happens |
| --- | --- |
| Discovery | Analyzes workspace symbols, tests, and git status |
| Execution | Performs edits and shell operations in bounded turns |
| Gate check | Verifies formatting, linting, and test suites |
| Delivery | Commits and submits changes cleanly |

Coder keeps you informed at each step, giving you visibility into every tool call, diff, and command output.
