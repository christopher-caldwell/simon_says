#!/usr/bin/env python3
"""Run a bounded existing Manabrew native diagnostic in an isolated action store."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


def load(path):
    return json.loads(Path(path).read_text())


def git(*args):
    return subprocess.check_output(["git", *args], text=True).strip()


def reply(packet, outcome, report, candidate=None, inspected=None):
    print(json.dumps({"action_id": packet["action_id"], "outcome": outcome,
                      "report": report, "candidate_commit": candidate, "inspected_commit": inspected}))


def invoke(cli, store, *args):
    process = subprocess.run([str(cli), "--local", str(store), *args], text=True,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=90)
    if process.returncode:
        raise RuntimeError(f"native simulator exited {process.returncode}: {process.stderr[-2000:]}")
    return json.loads(process.stdout)


def summary(simulation):
    return {"run": simulation["run"], "eligibleGames": simulation["eligibleGames"],
            "summary": "One native diagnostic game was verified; it does not establish deck quality or certify all game rules."}


def main():
    packet_path = Path(os.environ["SIMON_PACKET"])
    packet = load(packet_path)
    runtime_root = Path(os.environ["SIMON_MAGIC_RUNTIME_ROOT"]).resolve(strict=True)
    cli = runtime_root / "repo/sim/playtest"
    gate, stage, feedback = packet["gate"], packet["stage"], packet.get("feedback", [])
    if gate == "unblocker":
        reply(packet, "retry", "The isolated diagnostic may retry once.")
        return
    if gate == "worker" and stage == "simulate":
        if not any("isolated diagnostic may retry" in item for item in feedback):
            reply(packet, "blocked", "Exercise a recoverable preflight blocker before simulation.")
            return
        revision = 2 if any("revision 2" in item for item in feedback) else 1
        store = packet_path.parent / "native-store"
        store.mkdir()
        shutil.copyfile(runtime_root / "store/runtime.json", store / "runtime.json")
        result = invoke(cli, store, "run", "--config", str(Path("diagnostic.json").resolve()), "--repo", str(Path.cwd()))
        if result.get("state") != "complete" or result.get("assignedTrials") != 1 or result.get("eligibleGames") != 1:
            raise RuntimeError("native diagnostic did not complete with one eligible game")
        value = {"run": result["run"], "store": str(store), "assignedTrials": 1,
                 "eligibleGames": 1, "revision": revision, "rulesCertified": False}
        filename = "simulation.json"
    elif gate == "worker" and stage == "analyze":
        value, filename = summary(load("simulation.json")), "analysis.json"
    elif gate == "reviewer":
        head = git("rev-parse", "HEAD")
        if stage == "simulate":
            simulation = load("simulation.json")
            verification = invoke(cli, Path(simulation["store"]), "verify", "--run", simulation["run"])
            if verification.get("verified") != 1 or verification.get("eligibleGames") != 1:
                raise RuntimeError("native verification did not find one eligible verified game")
            if simulation["revision"] < 2:
                reply(packet, "changes_required", "Exercise correction: produce revision 2 and verify a new diagnostic.", inspected=head)
            else:
                reply(packet, "pass", "Native verifier confirmed one eligible game; rule completeness is not certified.", inspected=head)
        elif stage == "analyze":
            actual = load("analysis.json")
            reply(packet, "pass" if actual == summary(load("simulation.json")) else "changes_required",
                  "Checked the summary against the recorded native diagnostic.", inspected=head)
        else:
            raise ValueError("unsupported reviewer stage")
        return
    else:
        raise ValueError("unsupported action")
    Path(filename).write_text(json.dumps(value, indent=2) + "\n")
    subprocess.run(["git", "add", "--", filename], check=True)
    subprocess.run(["git", "-c", "user.name=Simon Says Native Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-m", f"Create {filename}"], stdout=subprocess.DEVNULL, check=True)
    reply(packet, "complete", f"Created {filename}.", candidate=git("rev-parse", "HEAD"))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"native diagnostic action failed: {error}", file=sys.stderr)
        sys.exit(1)
