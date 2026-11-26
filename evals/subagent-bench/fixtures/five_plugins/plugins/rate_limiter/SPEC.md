# Plugin: `plugins.rate_limiter`

Implement `TokenBucketLimiter` and `SlidingWindowCounter` in `plugins/rate_limiter/__init__.py`:
1. `TokenBucketLimiter(capacity: float, refill_per_sec: float)`:
   - Per-key buckets start full at `capacity` on first sight.
   - `allow(key: str, now_s: float, cost: float = 1.0) -> tuple[bool, float]`:
     - Refills `elapsed * refill_per_sec` capped at `capacity` (ignore negative elapsed).
     - If `tokens >= cost`, subtracts `cost` and returns `(True, 0.0)`.
     - Otherwise does NOT consume tokens and returns `(False, retry_after_s)` where `retry_after_s = (cost - tokens) / refill_per_sec`.
     - Raise `ValueError` if `cost <= 0` or `cost > capacity`.
2. `SlidingWindowCounter(limit: int, window_s: float)`:
   - Weighted two-bucket sliding window approximation (fixed sub-windows of length `window_s`):
     - `curr_bucket = int(now_s // window_s)`, `elapsed_frac = (now_s - curr_bucket * window_s) / window_s`.
     - Effective count = `prev_bucket_count * (1.0 - elapsed_frac) + curr_bucket_count` (where `prev_bucket_count` is `0` if previous bucket is not `curr_bucket - 1`).
   - `allow(key: str, now_s: float) -> bool`: if `effective + 1.0 <= limit + 1e-9`, increment current bucket count and return `True`, else `False`.
