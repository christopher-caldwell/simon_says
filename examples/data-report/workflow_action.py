#!/usr/bin/env python3
"""Deterministic local example of Simon Says' action-envelope protocol."""
import json
import os
import subprocess
import sys
from pathlib import Path


def output(packet, outcome, report, *, candidate=None, inspected=None):
    print(json.dumps({"action_id": packet["action_id"], "outcome": outcome,
                      "report": report, "candidate_commit": candidate,
                      "inspected_commit": inspected}))


def load(path):
    return json.loads(Path(path).read_text())


def expected_validation():
    data = load("input.json")
    rows = data.get("measurements")
    if not isinstance(rows, list) or not rows:
        raise ValueError("measurements must be a nonempty array")
    names, values = set(), []
    for row in rows:
        if not isinstance(row, dict):
            raise ValueError("each measurement must be an object")
        name, value = row.get("name"), row.get("value")
        if not isinstance(name, str) or not name.strip() or name in names:
            raise ValueError("measurement names must be nonempty and distinct")
        if type(value) is not int:
            raise ValueError("measurement values must be integers")
        names.add(name)
        values.append(value)
    return {"count": len(values), "total": sum(values), "minimum": min(values), "maximum": max(values)}


def expected_summary():
    values = load("validation.json")
    if values != expected_validation():
        raise ValueError("validation output does not match the input")
    mean = values["total"] / values["count"]
    return {"summary": f'{values["count"]} measurements; range {values["minimum"]} to {values["maximum"]}.', "mean": mean}


def git(*args):
    return subprocess.check_output(["git", *args], text=True).strip()


def main():
    packet = load(os.environ["SIMON_PACKET"])
    gate, stage = packet["gate"], packet["stage"]
    if gate == "unblocker":
        output(packet, "blocked", "This deterministic fixture cannot repair invalid input automatically.")
        return
    if stage not in ("validate", "report"):
        raise ValueError(f"unknown stage {stage!r}")
    path = "validation.json" if stage == "validate" else "summary.json"
    try:
        expected = expected_validation() if stage == "validate" else expected_summary()
    except (ValueError, OSError, json.JSONDecodeError, KeyError) as error:
        output(packet, "blocked", str(error), inspected=git("rev-parse", "HEAD") if gate == "reviewer" else None)
        return
    if gate == "worker":
        Path(path).write_text(json.dumps(expected, indent=2) + "\n")
        subprocess.run(["git", "add", "--", path], check=True)
        subprocess.run(["git", "-c", "user.name=Simon Says Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-m", f"Create {path}"], check=True, stdout=subprocess.DEVNULL)
        output(packet, "complete", f"Created {path} from the declared input.", candidate=git("rev-parse", "HEAD"))
    elif gate == "reviewer":
        inspected = git("rev-parse", "HEAD")
        try:
            actual = load(path)
        except (OSError, json.JSONDecodeError):
            actual = None
        if actual != expected:
            output(packet, "changes_required", f"{path} does not match the declared input.", inspected=inspected)
        else:
            output(packet, "pass", f"Checked {path} against the declared input.", inspected=inspected)
    else:
        raise ValueError(f"unknown gate {gate!r}")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"fixture action failed: {error}", file=sys.stderr)
        sys.exit(1)
