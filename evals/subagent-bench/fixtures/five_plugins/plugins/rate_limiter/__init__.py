# Firm reason: 5 (subagent-bench eval fixture/verifier)
class TokenBucketLimiter:
    def __init__(self, capacity: float, refill_per_sec: float):
        raise NotImplementedError


class SlidingWindowCounter:
    def __init__(self, limit: int, window_s: float):
        raise NotImplementedError
