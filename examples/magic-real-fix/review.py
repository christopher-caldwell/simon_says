import json,os,subprocess,sys
from pathlib import Path
p=json.loads(Path(os.environ["SIMON_PACKET"]).read_text()); head=subprocess.check_output(["git","rev-parse","HEAD"],text=True).strip()
if p["gate"] != "reviewer" or head != p["candidate_commit"]: raise ValueError("candidate identity mismatch")
changed=subprocess.check_output(["git","diff","--name-only",p["baseline_commit"],head],text=True).splitlines()
if any(x != "repair.json" and not x.startswith("manabrew-rs/crates/manabrew-engine/") for x in changed): raise ValueError("repair exceeds scoped engine files")
if "manabrew-rs/crates/manabrew-engine/tests/left_battlefield_this_turn.rs" not in changed: raise ValueError("missing requested executable regression")
env=dict(os.environ,CARGO_TARGET_DIR=os.path.join(os.environ["SIMON_SCRATCH"],"target"),DYLD_SHARED_REGION="avoid")
r=subprocess.run(["cargo","test","-p","manabrew-engine","--test","left_battlefield_this_turn"],env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=1700)
Path(os.environ["SIMON_SCRATCH"],"review-tests.txt").write_text(r.stdout)
ok=r.returncode==0 and "test result: ok." in r.stdout and "running 0 tests" not in r.stdout
print(json.dumps({"action_id":p["action_id"],"outcome":"pass" if ok else "changes_required","report":"Exact candidate passed the scoped executable regression; no runtime installation or rules certification." if ok else "Executable regression failed: "+r.stdout[-3000:],"candidate_commit":None,"inspected_commit":head}))
