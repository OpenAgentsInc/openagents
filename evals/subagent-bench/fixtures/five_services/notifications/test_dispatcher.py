# Firm reason: 5 (subagent-bench eval fixture/verifier)
import unittest
from notifications.dispatcher import NotificationDispatcher, TemplateError, render_template


class TestNotificationsDispatcher(unittest.TestCase):
    def test_template_escaping_and_strict_keys(self):
        out = render_template("Hello {{ name }}!", {"name": "<script>x</script>"}, escape_html=True)
        self.assertEqual(out, "Hello &lt;script&gt;x&lt;/script&gt;!")
        raw = render_template("Hi {{name}}", {"name": "<b>A</b>"}, escape_html=False)
        self.assertEqual(raw, "Hi <b>A</b>")
        with self.assertRaises(TemplateError):
            render_template("Hi {{ missing_key }}", {"name": "A"})

    def test_sliding_window_boundary_and_urgent_bypass(self):
        d = NotificationDispatcher(max_per_window=2, window_s=60)
        self.assertEqual(d.send("u1", "M1 {{x}}", {"x": "1"}, now_s=100), "sent")
        self.assertEqual(d.send("u1", "M2 {{x}}", {"x": "2"}, now_s=120), "sent")
        self.assertEqual(d.send("u1", "M3 {{x}}", {"x": "3"}, now_s=159), "rate_limited")
        # Urgent bypasses rate limit
        self.assertEqual(d.send("u1", "ALERT {{x}}", {"x": "9"}, now_s=159, urgent=True), "sent")
        # At now_s=160, t=100 is exactly window_s=60 old and must expire; however urgent counted at 159 or didn't?
        # Wait: let's test fresh user u2 for exact boundary without urgent interference
        self.assertEqual(d.send("u2", "A", {}, now_s=100), "sent")
        self.assertEqual(d.send("u2", "B", {}, now_s=130), "sent")
        self.assertEqual(d.send("u2", "C", {}, now_s=159), "rate_limited")
        self.assertEqual(d.send("u2", "D", {}, now_s=160), "sent")

    def test_idempotency_dedup_ttl(self):
        d = NotificationDispatcher(max_per_window=5, window_s=60, dedup_ttl_s=100)
        self.assertEqual(d.send("u1", "Hi", {}, now_s=10, idempotency_key="k1"), "sent")
        self.assertEqual(d.send("u1", "Hi", {}, now_s=109, idempotency_key="k1"), "deduplicated")
        self.assertEqual(d.send("u1", "Hi", {}, now_s=110, idempotency_key="k1"), "sent")


if __name__ == "__main__":
    unittest.main()
