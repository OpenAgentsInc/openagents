"""
OpenAgents Harbor - Agent adapters for Terminal-Bench evaluations.

Provides Harbor adapters for:
- ClaudeCodeAgent: Claude Code CLI with testgen skill
- PiAgent: Pi coding agent

Usage:
    harbor run --agent-import-path openagents_harbor:ClaudeCodeAgent ...
    harbor run --agent-import-path openagents_harbor:PiAgent ...
"""

from openagents_harbor.claude_agent import ClaudeCodeAgent
from openagents_harbor.pi_agent import PiAgent

__all__ = ["ClaudeCodeAgent", "PiAgent"]
