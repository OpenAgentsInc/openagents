def compute_backoff(attempt: int, base_ms: int = 100) -> int:
    if attempt <= 0:
        return 0
    return base_ms * (2 ** (attempt - 1))
