import json
import os
from pathlib import Path
packet = json.loads(Path(os.environ["SIMON_PACKET"]).read_text())
print(json.dumps({"action_id": packet["action_id"], "outcome": "blocked", "report": "Stop for inspection; this repair fixture is not authorized to expand scope or retry uncertain external actions.", "candidate_commit": None, "inspected_commit": None}))
