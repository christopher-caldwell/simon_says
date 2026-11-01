#!/usr/bin/env python3
"""Independent mechanical contract verifier; not a domain-judgment substitute."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys


def load(path):
    return json.loads(Path(path).read_text())


class ChangesRequired(Exception):
    pass


def main():
    packet = load(os.environ["SIMON_PACKET"])
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    if packet["gate"] != "reviewer" or head != packet["candidate_commit"]:
        raise ValueError("not reviewing the recorded candidate")
    stage = packet["stage"]
    outcome = "pass"
    try:
        report = review_stage(stage, packet)
    except ChangesRequired as error:
        outcome, report = "changes_required", str(error)
    except Exception as error:
        if stage != "supervise":
            raise
        outcome, report = "blocked", f"Native evidence could not be verified; no retry is authorized: {error}"
    print(json.dumps({"action_id": packet["action_id"], "outcome": outcome, "report": report,
                      "candidate_commit": None, "inspected_commit": head}))


def review_stage(stage, packet):
    if stage == "investigate":
        design = load("design.json")
        if design["original_question"] != packet.get("run_context") or design["config_sha256"] != hashlib.sha256(Path("diagnostic.json").read_bytes()).hexdigest():
            raise ChangesRequired("Correct the design to preserve the exact original question and config SHA-256.")
        if design["planned_games"] != 4:
            raise ChangesRequired("The design must specify exactly four isolated diagnostic games.")
        report = "Confirmed the design binds the original question and exact four-game configuration."
    elif stage == "supervise":
        collection = load("collection.json")
        if collection["original_question"] != packet.get("run_context"):
            raise ValueError("collection lost the original question")
        from native_tool import mirror_verify
        result = mirror_verify(collection)
        report = f"Native verifier confirmed {result['verified']} retained attempts and {result['eligibleGames']} eligible games; no claim of rules certification."
    elif stage == "analyze":
        analysis = load("analysis.json")
        collection = load("collection.json")
        errors = []
        if analysis["original_question"] != packet.get("run_context"):
            errors.append("set original_question exactly to packet.run_context")
        if analysis["verified_games"] != collection["eligible"]:
            errors.append("set verified_games to collection.eligible (eligible games, not verified attempts)")
        if analysis["issues"] != collection["issues"]:
            errors.append("set issues exactly to collection.issues; put supplemental detail in answer or limitations")
        if errors:
            raise ChangesRequired("; ".join(errors))
        report = "Mechanically checked original question, verified count and issues; this check does not certify the Analyzer's judgment."
    else:
        raise ValueError("unknown stage")
    return report


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"mechanical verifier failed: {error}", file=sys.stderr)
        sys.exit(1)
