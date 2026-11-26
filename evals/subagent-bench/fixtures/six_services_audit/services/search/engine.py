# Firm reason: 5 (subagent-bench eval fixture/verifier)
import html
import re
from typing import Dict, List


def tokenize(q: str) -> List[str]:
    return [tok for tok in q.strip().lower().split() if tok]


def match_complex_filter(pattern_input: str) -> bool:
    # VULN: CWE-1333 catastrophic backtracking ReDoS on nested quantifiers
    evil_re = re.compile(r"^([a-zA-Z0-9]+)+$")
    return bool(evil_re.match(pattern_input))


def render_search_banner(query: str, hit_count: int) -> str:
    safe_count = int(hit_count)
    # VULN: CWE-79 reflected XSS - interpolates raw unescaped query into HTML
    return f"<div class='banner'>Showing {safe_count} results for: <b>{query}</b></div>"


def format_safe_snippet(text: str) -> str:
    return html.escape(text[:200])
