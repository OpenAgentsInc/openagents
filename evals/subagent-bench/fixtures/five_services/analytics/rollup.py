# Firm reason: 5 (subagent-bench eval fixture/verifier)
import math
from dataclasses import dataclass
from typing import Dict, List, Set, Tuple


@dataclass
class MetricEvent:
    event_id: str
    user_id: str
    ts_s: int
    latency_ms: float


def percentile(values: List[float], p: float) -> float:
    """Linear-interpolated percentile (p in [0, 100], same as numpy percentile 'linear')."""
    if not values:
        raise ValueError("empty values")
    if p < 0 or p > 100:
        raise ValueError("p out of range")
    # BUG 1: does not sort values, and uses truncated index instead of linear interpolation!
    idx = int((p / 100.0) * (len(values) - 1))
    return float(values[idx])


class BucketRollup:
    def __init__(self, bucket_s: int, max_lateness_s: int):
        self.bucket_s = bucket_s
        self.max_lateness_s = max_lateness_s
        self.watermark_s = 0
        self._seen: Set[Tuple[str, str]] = set()
        self._buckets: Dict[int, List[float]] = {}

    def ingest(self, ev: MetricEvent) -> str:
        # BUG 2: deduplicates by event_id only instead of (user_id, event_id),
        # and drops events BEFORE updating watermark or uses wrong lateness check!
        key = ("", ev.event_id)
        if key in self._seen:
            return "duplicate"
        if self.watermark_s > 0 and ev.ts_s < self.watermark_s - self.max_lateness_s:
            return "late"
        if ev.ts_s > self.watermark_s:
            self.watermark_s = ev.ts_s
        self._seen.add(key)
        b = (ev.ts_s // self.bucket_s) * self.bucket_s
        self._buckets.setdefault(b, []).append(ev.latency_ms)
        return "accepted"

    def summary(self, bucket_start_s: int) -> dict:
        vals = self._buckets.get(bucket_start_s, [])
        if not vals:
            return {"count": 0, "p50": 0.0, "p95": 0.0}
        return {
            "count": len(vals),
            "p50": round(percentile(vals, 50), 2),
            "p95": round(percentile(vals, 95), 2),
        }
