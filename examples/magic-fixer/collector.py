"""Small isolated collector adapter, deliberately defective for FIXTURE-001."""

def eligible_attempts(attempts):
    # BUG: retained does not mean eligible. Rejected attempts can be retained.
    return [attempt for attempt in attempts if attempt.get("retained") is True]
