# Firm reason: 5 (subagent-bench eval fixture/verifier)
import html
import re
from typing import Dict, List, Optional, Tuple


PLACEHOLDER_RE = re.compile(r"\{\{\s*([a-zA-Z0-9_]+)\s*\}\}")


class TemplateError(KeyError):
    pass


def render_template(template: str, variables: Dict[str, str], escape_html: bool = True) -> str:
    # BUG 1: silently replaces missing keys with empty string instead of raising TemplateError,
    # and forgets to HTML-escape substituted values when escape_html=True!
    def _sub(match: re.Match) -> str:
        key = match.group(1)
        return str(variables.get(key, ""))
    return PLACEHOLDER_RE.sub(_sub, template)


class NotificationDispatcher:
    def __init__(self, max_per_window: int, window_s: int, dedup_ttl_s: int = 300):
        self.max_per_window = max_per_window
        self.window_s = window_s
        self.dedup_ttl_s = dedup_ttl_s
        self._sent_times: Dict[str, List[int]] = {}
        self._dedup: Dict[str, int] = {}
        self.deliveries: List[Tuple[str, str]] = []

    def send(
        self,
        recipient: str,
        template: str,
        variables: Dict[str, str],
        now_s: int,
        idempotency_key: Optional[str] = None,
        urgent: bool = False,
        escape_html: bool = True,
    ) -> str:
        # Check idempotency key TTL
        if idempotency_key is not None:
            exp = self._dedup.get(idempotency_key)
            if exp is not None and now_s < exp:
                return "deduplicated"

        # Sliding window rate limit per recipient: strictly within (now_s - window_s, now_s]
        # BUG 2: uses `<` instead of `<=` on pruning boundary, AND blocks urgent notifications instead of bypassing limit!
        hist = self._sent_times.setdefault(recipient, [])
        hist[:] = [t for t in hist if t >= now_s - self.window_s]
        if len(hist) >= self.max_per_window:
            return "rate_limited"

        body = render_template(template, variables, escape_html=escape_html)
        hist.append(now_s)
        if idempotency_key is not None:
            self._dedup[idempotency_key] = now_s + self.dedup_ttl_s
        self.deliveries.append((recipient, body))
        return "sent"
