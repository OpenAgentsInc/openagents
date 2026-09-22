"""Local Terminal-Bench comparisons for Claude Code, Codex, and Coder v0.5.

This package is benchmark infrastructure: a thin adapter over Harbor's
installed-agent interface plus the repository's own profiles, attempt
records, and reports. Rust remains the product implementation language;
nothing here is a second agent implementation.
"""

__version__ = "0.1.0"

# The Harbor release every profile and adapter in this package is written
# against. The pin lives in pyproject.toml; this constant is what doctor
# checks the installed package against.
HARBOR_PIN = "0.22.0"

# The upstream terminal-bench commit the task panel is drawn from.
UPSTREAM_COMMIT = "3b5caaa4863d64dda7f0957bf4fc2d4f019202d4"
UPSTREAM_GIT_URL = "https://github.com/harbor-framework/terminal-bench.git"
