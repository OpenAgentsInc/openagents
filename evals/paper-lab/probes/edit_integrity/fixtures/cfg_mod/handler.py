# Firm reason: 5 (Paper-Lab eval fixture: a tiny Python project the live agent edits in a temp repo)
def compute_backoff(attempt: int, base_ms: int = 100) -> int:
    if attempt <= 0:
        return 0
    return base_ms * (2 ** (attempt - 1))
