#!/usr/bin/env python3
"""
DEPRECATED: Demo gallery moved to ~/code/backroom
This script is no longer used. Demo and demos folders have been archived.
"""

import sys
print("ERROR: Demo gallery has been moved to ~/code/backroom")
print("This script is deprecated and should not be run.")
sys.exit(1)

import os
import re
import json
from pathlib import Path
from dataclasses import dataclass, field
from typing import List, Optional

@dataclass
class SessionMetrics:
    """Metrics for evaluating demo quality."""
    file_path: Path
    size_kb: float
    tokens_in: int = 0
    tokens_out: int = 0
    tokens_cached: int = 0

    # Tool usage diversity
    tools_used: List[str] = field(default_factory=list)
    read_count: int = 0
    edit_count: int = 0
    write_count: int = 0
    bash_count: int = 0
    grep_count: int = 0

    # Outcomes
    git_commits: int = 0
    files_modified: int = 0
    tests_run: bool = False
    build_errors: int = 0

    # Completion status
    completed: bool = False
    error_count: int = 0

    def quality_score(self) -> float:
        """Calculate overall quality score (0-100)."""
        score = 0.0

        # Size/engagement (0-25 points)
        if 20 <= self.size_kb <= 200:
            score += 25
        elif 10 <= self.size_kb <= 300:
            score += 15

        # Token output (0-20 points)
        if 5000 <= self.tokens_out <= 50000:
            score += 20
        elif 1000 <= self.tokens_out <= 100000:
            score += 10

        # Tool diversity (0-25 points)
        tool_variety = len(set(self.tools_used))
        score += min(25, tool_variety * 3)

        # Code changes (0-15 points)
        if self.edit_count > 0:
            score += 10
        if self.git_commits > 0:
            score += 5

        # Success indicators (0-15 points)
        if self.completed:
            score += 10
        if self.error_count == 0:
            score += 5

        # Penalty for errors
        score -= min(15, self.error_count * 2)

        return max(0, min(100, score))

def parse_rlog_metadata(rlog_path: Path) -> SessionMetrics:
    """Extract metadata from .rlog file."""
    metrics = SessionMetrics(
        file_path=rlog_path,
        size_kb=os.path.getsize(rlog_path) / 1024
    )

    try:
        with open(rlog_path, 'r') as f:
            # Read header (YAML frontmatter)
            in_header = False
            for line in f:
                if line.strip() == '---':
                    if in_header:
                        break
                    in_header = True
                    continue

                if in_header:
                    if match := re.match(r'^(\w+):\s*(.+)', line):
                        key, value = match.groups()
                        if key == 'tokens_total_in':
                            metrics.tokens_in = int(value)
                        elif key == 'tokens_total_out':
                            metrics.tokens_out = int(value)
                        elif key == 'tokens_cached':
                            metrics.tokens_cached = int(value)

            # Scan events for tool usage
            f.seek(0)
            for line in f:
                # Tool calls
                if match := re.search(r't!:(\w+)', line):
                    tool = match.group(1)
                    metrics.tools_used.append(tool)

                    if tool == 'Read':
                        metrics.read_count += 1
                    elif tool == 'Edit':
                        metrics.edit_count += 1
                    elif tool == 'Write':
                        metrics.write_count += 1
                    elif tool == 'Bash':
                        metrics.bash_count += 1
                    elif tool == 'Grep':
                        metrics.grep_count += 1

                # Git commits
                if 'git commit' in line:
                    metrics.git_commits += 1

                # Tests
                if re.search(r'cargo test|pytest|npm test', line):
                    metrics.tests_run = True

                # Build errors
                if 'error:' in line.lower() or 'error[E' in line:
                    metrics.build_errors += 1

                # Error outcomes
                if '→ [error]' in line:
                    metrics.error_count += 1

                # Completion
                if 'issue_complete' in line or 'Completed issue' in line:
                    metrics.completed = True

    except Exception as e:
        print(f"Error parsing {rlog_path}: {e}")

    return metrics

def main():
    """Analyze all .rlog files and select top demos."""
    logs_dir = Path("docs/logs")

    if not logs_dir.exists():
        print(f"Error: {logs_dir} not found")
        return

    print("Analyzing .rlog sessions...")

    # Find all .rlog files
    rlog_files = sorted(logs_dir.rglob("*.rlog"), key=os.path.getmtime, reverse=True)
    print(f"Found {len(rlog_files)} total sessions\n")

    # Parse and score all sessions
    sessions = []
    for rlog in rlog_files:
        metrics = parse_rlog_metadata(rlog)
        sessions.append(metrics)

    # Sort by quality score
    sessions.sort(key=lambda s: s.quality_score(), reverse=True)

    # Display top 20
    print("=" * 80)
    print("TOP 20 DEMO CANDIDATES")
    print("=" * 80)
    print(f"{'Rank':<6} {'Score':<7} {'Size':<10} {'Tokens':<12} {'Tools':<8} {'Edits':<7} {'Path':<30}")
    print("-" * 80)

    for i, session in enumerate(sessions[:20], 1):
        relative_path = session.file_path.relative_to(logs_dir)
        tool_count = len(set(session.tools_used))

        print(f"{i:<6} {session.quality_score():<7.1f} {session.size_kb:<10.1f} "
              f"{session.tokens_out:<12} {tool_count:<8} {session.edit_count:<7} "
              f"{str(relative_path)[:30]}")

    print("\n" + "=" * 80)
    print("RECOMMENDED FOR DEMO GALLERY (Top 5)")
    print("=" * 80)

    for i, session in enumerate(sessions[:5], 1):
        print(f"\n{i}. {session.file_path.relative_to(logs_dir)}")
        print(f"   Quality Score: {session.quality_score():.1f}/100")
        print(f"   Size: {session.size_kb:.1f} KB")
        print(f"   Tokens: {session.tokens_out:,} out")
        unique_tools = list(set(session.tools_used))
        print(f"   Tools: {len(unique_tools)} unique ({', '.join(unique_tools[:10])})")
        print(f"   Edits: {session.edit_count} | Commits: {session.git_commits} | Errors: {session.error_count}")
        print(f"   Completed: {'✅' if session.completed else '❌'}")
        print(f"   Tests Run: {'✅' if session.tests_run else '❌'}")

    # Save results to JSON
    output_file = Path("demo_selection.json")
    top_5 = [
        {
            "rank": i + 1,
            "file": str(s.file_path.relative_to(logs_dir)),
            "score": round(s.quality_score(), 1),
            "size_kb": round(s.size_kb, 1),
            "tokens_out": s.tokens_out,
            "tools": len(set(s.tools_used)),
            "edits": s.edit_count,
            "commits": s.git_commits,
            "completed": s.completed,
            "tests_run": s.tests_run
        }
        for i, s in enumerate(sessions[:5])
    ]

    with open(output_file, 'w') as f:
        json.dump({"top_5_demos": top_5}, f, indent=2)

    print(f"\n✅ Results saved to {output_file}")

if __name__ == "__main__":
    main()
