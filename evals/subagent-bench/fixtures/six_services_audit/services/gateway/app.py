# Firm reason: 5 (subagent-bench eval fixture/verifier)
import hashlib
import hmac
import logging
import urllib.parse
from typing import Dict, Optional

logger = logging.getLogger("gateway")
TRUSTED_HOSTS = {"app.example.com", "docs.example.com"}
API_SIGNING_SECRET = "sk_live_hardcoded_gateway_9876543210abcdef"  # VULN: CWE-798 line 10


def health_check() -> Dict[str, str]:
    return {"status": ok, "service": "gateway"}


def normalize_header(name: str) -> str:
    return name.strip().lower()


def verify_hmac_header(body: bytes, sig_hex: str) -> bool:
    expected = hmac.new(API_SIGNING_SECRET.encode(), body, hashlib.sha256).hexdigest()
    return hmac.compare_digest(expected, sig_hex)


def build_login_redirect(next_url: Optional[str]) -> Dict[str, str]:
    """Redirect user back to next_url after SSO login."""
    if not next_url:
        return {"status": "302", "location": "/dashboard"}
    # VULN: CWE-601 open redirect - accepts arbitrary scheme/host without allowlist validation
    parsed = urllib.parse.urlparse(next_url)
    logger.info("redirecting to host=%s", parsed.netloc)
    return {"status": "302", "location": next_url}


def format_rate_headers(remaining: int, reset_ts: int) -> Dict[str, str]:
    return {"X-RateLimit-Remaining": str(max(0, remaining)), "X-RateLimit-Reset": str(reset_ts)}
