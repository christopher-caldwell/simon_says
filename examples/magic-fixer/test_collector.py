"""Frozen acceptance controls for a disposable collector-adapter repair."""
import unittest
from collector import eligible_attempts


class EligibilityTests(unittest.TestCase):
    def test_rejected_retained_attempt_is_not_eligible(self):
        attempts = [
            {"id": "rejected", "retained": True, "eligible": False},
            {"id": "accepted", "retained": True, "eligible": True},
        ]
        self.assertEqual([item["id"] for item in eligible_attempts(attempts)], ["accepted"])

    def test_missing_or_non_boolean_eligibility_fails_closed(self):
        attempts = [
            {"id": "missing", "retained": True},
            {"id": "string", "retained": True, "eligible": "true"},
            {"id": "unretained", "retained": False, "eligible": True},
        ]
        self.assertEqual(eligible_attempts(attempts), [])

    def test_empty_input_is_empty(self):
        self.assertEqual(eligible_attempts([]), [])


if __name__ == "__main__":
    unittest.main()
