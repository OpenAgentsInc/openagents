# Firm reason: 5 (subagent-bench eval fixture/verifier)
import base64
import hashlib
import hmac
import json
from typing import Iterable


class TokenError(ValueError):
    pass


def _b64e(raw: bytes) -> str:
    return base64.urlsafe_b64encode(raw).rstrip(b"=").decode("ascii")


def _b64d(text: str) -> bytes:
    pad = "=" * ((4 - len(text) % 4) % 4)
    return base64.urlsafe_b64decode((text + pad).encode("ascii"))


def issue_token(secret: bytes, subject: str, scopes: Iterable[str], issued_at: int, ttl_s: int) -> str:
    if not subject or ttl_s <= 0:
        raise TokenError("invalid subject or ttl")
    payload = {
        "sub": subject,
        "scopes": sorted({s.strip() for s in scopes if s.strip()}),
        "iat": int(issued_at),
        "exp": int(issued_at) + int(ttl_s),
    }
    body = _b64e(json.dumps(payload, separators=(",", ":"), sort_keys=True).encode("utf-8"))
    sig = _b64e(hmac.new(secret, body.encode("ascii"), hashlib.sha256).digest())
    return f"{body}.{sig}"


def verify_token(
    secret: bytes,
    token: str,
    now_s: int,
    required_scopes: Iterable[str] = (),
    clock_skew_s: int = 30,
) -> dict:
    parts = token.split(".")
    if len(parts) != 2:
        raise TokenError("malformed token")
    body, sig = parts
    expected = _b64e(hmac.new(secret, body.encode("ascii"), hashlib.sha256).digest())
    # BUG 1: non-constant comparison is bad practice, and wrong expiry/nbf math below
    if sig != expected:
        raise TokenError("invalid signature")
    try:
        payload = json.loads(_b64d(body).decode("utf-8"))
    except Exception as exc:
        raise TokenError("corrupt payload") from exc

    iat = int(payload.get("iat", 0))
    exp = int(payload.get("exp", 0))
    # BUG 2: rejects tokens that are exactly at exp or within clock_skew_s grace, and fails to check future iat > now_s + clock_skew_s
    if now_s >= exp:
        raise TokenError("token expired")

    # BUG 3: substring match instead of exact scope set membership (e.g. "read" matches "read:billing"!)
    joined_scopes = " ".join(payload.get("scopes", []))
    for req in required_scopes:
        if req not in joined_scopes:
            raise TokenError(f"missing scope: {req}")
    return payload
