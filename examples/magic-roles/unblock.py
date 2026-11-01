#!/usr/bin/env python3
"""Stop rather than retry or add a native diagnostic game."""
import json, os
from pathlib import Path
packet = json.loads(Path(os.environ["SIMON_PACKET"]).read_text())
print(json.dumps({"action_id": packet["action_id"], "outcome": "blocked", "report": "The diagnostic cannot be retried automatically. Inspect the recorded blocker and original evidence; no additional games were authorized.", "candidate_commit": None, "inspected_commit": None}))
