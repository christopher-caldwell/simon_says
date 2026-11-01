import json,os
from pathlib import Path
p=json.loads(Path(os.environ["SIMON_PACKET"]).read_text())
print(json.dumps({"action_id":p["action_id"],"outcome":"blocked","report":"Stop for explicit inspection; do not replay an uncertain repair action.","candidate_commit":None,"inspected_commit":None}))
