#!/usr/bin/env python3
"""Deterministic role actions for the generic dynamic-routing example."""
import json
import os
from pathlib import Path
import subprocess
import sys


def read(path):
    return json.loads(Path(path).read_text())


def git(*args):
    return subprocess.check_output(["git", *args], text=True).strip()


def commit(paths, message):
    subprocess.run(["git", "add", "--", *paths], check=True)
    subprocess.run(["git", "-c", "user.name=Dynamic Repair", "-c", "user.email=fixture@example.invalid", "commit", "-qm", message], check=True)
    return git("rev-parse", "HEAD")


def emit(packet, outcome, report, candidate=None, inspected=None, route=None):
    response = {"action_id": packet["action_id"], "outcome": outcome, "report": report,
                "candidate_commit": candidate, "inspected_commit": inspected}
    if route is not None:
        response["route"] = route
    print(json.dumps(response))


def main():
    role = sys.argv[1]
    packet = read(os.environ["SIMON_PACKET"])
    stage, gate = packet["stage"], packet["gate"]
    engine, mode = read("engine.json"), read("mode.json")["mode"]
    if gate == "unblocker" and role == "stop":
        return emit(packet, "blocked", "The deterministic fixture cannot resolve this blocker.")
    if gate == "worker" and stage == "simulate" and role == "simulator":
        previous = read("simulation.json")["attempt"] if Path("simulation.json").exists() else 0
        probe = subprocess.run(["python3", "-B", "probe.py"], text=True, capture_output=True)
        if probe.returncode not in (0, 7):
            return emit(packet, "blocked", f"Probe failed unexpectedly: exit {probe.returncode}; {probe.stderr}")
        issue = probe.returncode == 7
        if not issue and probe.stdout.strip() != "42":
            return emit(packet, "blocked", "Probe returned an unexpected result.")
        result = {"issue": issue, "attempt": previous + 1,
                  "observed_value": 0 if issue else 42}
        Path("simulation.json").write_text(json.dumps(result) + "\n")
        candidate = commit(["simulation.json"], "Record probe observation")
        return emit(packet, "complete", f"Probe exited {probe.returncode}: {probe.stderr.strip() or probe.stdout.strip()}", candidate=candidate, route="issues" if issue else "clean")
    if gate == "worker" and stage == "repair" and role == "repair_specialist":
        observation = read("simulation.json")
        if not observation["issue"]:
            raise ValueError("repair must receive an observed issue")
        previous = read("repair.json")["repair_attempt"] if Path("repair.json").exists() else 0
        fixed = mode != "permanent"
        Path("engine.json").write_text(json.dumps({"correct": fixed}) + "\n")
        Path("repair.json").write_text(json.dumps({"issue_observed": True, "repair_attempt": previous + 1, "fix_applied": fixed}) + "\n")
        candidate = commit(["engine.json", "repair.json"], "Repair reported issue")
        return emit(packet, "complete", "Attempted repair of the reported issue.", candidate=candidate)
    if gate == "reviewer" and role == "quality_reviewer":
        head = git("rev-parse", "HEAD")
        if head != packet["candidate_commit"]:
            raise ValueError("reviewer does not have the exact candidate")
        if stage == "simulate":
            result = read("simulation.json")
            valid = result["issue"] == (not engine["correct"]) and result["observed_value"] == (42 if engine["correct"] else 0)
            if not valid:
                return emit(packet, "changes_required", "Correct the observed result.", inspected=head)
            route = "issues" if result["issue"] else "clean"
            if packet["candidate_route"] != route:
                return emit(packet, "changes_required", "Correct the proposed route.", inspected=head)
            if mode == "unauthorized_route":
                route = "not_declared"
            elif mode == "missing_route":
                route = None
            return emit(packet, "pass", "Inspected the exact simulated observation.", inspected=head, route=None)
        if stage == "repair":
            repair, result = read("repair.json"), read("simulation.json")
            valid = repair["issue_observed"] and result["issue"] and repair["fix_applied"] == engine["correct"]
            return emit(packet, "pass" if valid else "changes_required", "Reviewed repair against the reported issue.", inspected=head)
    raise ValueError("unexpected stage, role or gate")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"fixture action failed: {error}", file=sys.stderr)
        sys.exit(1)
