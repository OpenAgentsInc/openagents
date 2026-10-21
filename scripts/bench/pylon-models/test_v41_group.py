import unittest

from v41_group import END_OF_SENTENCE, check, split


def score(name, text):
    return text.strip() == "373" if name == "arithmetic" else None


def answer(text, answer_ms=1500, finish="stop", ceiling=600, deadline=600, ok=True):
    return {"ok": ok, "data": {"text": text, "answer_ms": answer_ms, "declared": {
        "budget_secs": 3600, "ceiling_secs": ceiling, "deadline_secs": deadline,
        "admitted_secs": deadline, "heartbeats": 2, "finish": finish, "connections": 1}}}


class V41GroupTests(unittest.TestCase):
    def test_split_starts_in_reasoning_and_keeps_the_final_bytes(self):
        self.assertEqual(split("think</think>\n373 " + END_OF_SENTENCE), ("think", "\n373 "))
        self.assertEqual(split("<think>a</think>b"), ("a", "b"))
        # Unfinished reasoning never becomes the final answer.
        self.assertEqual(split("still thinking"), ("still thinking", None))
        # Only the first marker ends reasoning.
        self.assertEqual(split("a</think>b</think>c"), ("a", "b</think>c"))

    def test_a_complete_answer_within_the_deadline_passes(self):
        rows = check("arithmetic", answer("17 x 24 = 408.</think>373"), score)
        self.assertTrue(all(passed for _, passed, _ in rows), rows)

    def test_each_failure_is_named(self):
        def failed(response):
            return [name for name, passed, _ in check("arithmetic", response, score) if not passed]
        self.assertEqual(failed(answer("reasoning only")), ["reasoning_and_final", "final_scores"])
        self.assertEqual(failed(answer("r</think>374")), ["final_scores"])
        self.assertEqual(failed(answer("r</think>373", finish=None)), ["finish_stop"])
        self.assertEqual(failed(answer("r</think>373", answer_ms=20_001, deadline=20)),
                         ["within_declared_deadline"])
        missed = {"ok": False, "summary": "no pylon answered (x: limit_exceeded: the job ran out of time)"}
        self.assertIn("answered", failed(missed))
        self.assertIn("within_declared_deadline", failed(missed))


if __name__ == "__main__":
    unittest.main()
