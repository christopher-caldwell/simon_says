#!/usr/bin/env python3
"""Review the exact candidate with frozen, independently authored controls."""
import json
import os
from pathlib import Path
import subprocess

packet = json.loads(Path(os.environ["SIMON_PACKET"]).read_text())
head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
if packet["gate"] != "reviewer" or head != packet["candidate_commit"]:
    raise SystemExit("candidate identity mismatch")
result = subprocess.run(["python3", "-B", "-m", "unittest", "-v", "test_collector", "test_repair"], text=True, capture_output=True)
outcome = "pass" if result.returncode == 0 else "changes_required"
report = "Frozen fixture controls passed; no real engine, runtime, or installation was tested." if not result.returncode else "Correct the repair and tests: " + (result.stdout + result.stderr)[-4000:]
print(json.dumps({"action_id": packet["action_id"], "outcome": outcome, "report": report, "candidate_commit": None, "inspected_commit": head}))
