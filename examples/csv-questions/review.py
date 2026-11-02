#!/usr/bin/env python3
"""Mechanical review only; domain conclusions belong to the workflow's roles."""
import json
import os
from pathlib import Path
import subprocess

packet = json.loads(Path(os.environ["SIMON_PACKET"]).read_text())
response = {"action_id": packet["action_id"], "candidate_commit": None, "inspected_commit": None}
if packet["gate"] == "unblocker":
    response.update(outcome="blocked", report="The configured role could not complete this example.")
else:
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    response.update(outcome="pass" if head == packet["candidate_commit"] else "blocked",
                    report="Checked the exact candidate; artifact structure is validated by the controller.",
                    inspected_commit=head)
print(json.dumps(response))
