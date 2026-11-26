# Firm reason: 5 (subagent-bench eval fixture/verifier)
import unittest
from analytics.rollup import BucketRollup, MetricEvent, percentile


class TestAnalyticsRollup(unittest.TestCase):
    def test_linear_percentile_unsorted(self):
        vals = [40.0, 10.0, 50.0, 20.0, 30.0]
        self.assertAlmostEqual(percentile(vals, 50), 30.0, places=4)
        self.assertAlmostEqual(percentile(vals, 95), 48.0, places=4)
        self.assertAlmostEqual(percentile(vals, 25), 20.0, places=4)
        self.assertAlmostEqual(percentile([10.0, 20.0], 50), 15.0, places=4)

    def test_dedup_scoped_by_user_and_event(self):
        r = BucketRollup(bucket_s=60, max_lateness_s=30)
        self.assertEqual(r.ingest(MetricEvent("e1", "u1", 120, 10.0)), "accepted")
        self.assertEqual(r.ingest(MetricEvent("e1", "u1", 121, 99.0)), "duplicate")
        # Same event_id from different user is distinct!
        self.assertEqual(r.ingest(MetricEvent("e1", "u2", 122, 30.0)), "accepted")
        s = r.summary(120)
        self.assertEqual(s["count"], 2)
        self.assertAlmostEqual(s["p50"], 20.0, places=2)

    def test_watermark_lateness(self):
        r = BucketRollup(bucket_s=60, max_lateness_s=20)
        self.assertEqual(r.ingest(MetricEvent("e1", "u1", 200, 10.0)), "accepted")
        self.assertEqual(r.ingest(MetricEvent("e2", "u1", 180, 20.0)), "accepted")
        self.assertEqual(r.ingest(MetricEvent("e3", "u1", 179, 50.0)), "late")


if __name__ == "__main__":
    unittest.main()
