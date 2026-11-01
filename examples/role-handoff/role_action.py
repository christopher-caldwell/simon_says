#!/usr/bin/env python3
"""Deterministic role-handoff fixture; not an actual authorization service."""
import json
import os
from pathlib import Path
import subprocess
import sys


def read(path):
    return json.loads(Path(path).read_text())


def git(*args):
    return subprocess.check_output(["git", *args], text=True).strip()


def emit(packet, outcome, report, candidate=None, inspected=None):
    print(json.dumps({"action_id": packet["action_id"], "outcome": outcome,
                      "report": report, "candidate_commit": candidate,
                      "inspected_commit": inspected}))


def result(stage, packet):
    if stage == "investigate":
        q = read("question.json")
        if not isinstance(q.get("question"), str) or not q["question"].strip() or not isinstance(q.get("observations"), list):
            raise ValueError("invalid original question or observations")
        context = packet.get("run_context")
        if not isinstance(context, str) or context.strip() != q["question"]:
            raise ValueError("frozen original task does not match question evidence")
        return {"question": context.strip(), "observations": q["observations"], "evidence": "question.json"}
    if stage == "supervise":
        i, q = read("investigation.json"), read("question.json")
        return {"question": i["question"], "observations": i["observations"],
                "collection_evidence": "investigation.json", "analysis_performed": False,
                "backlog": [{"id": "MISSING_CONTEXT", "issue": q["metadata_warning"], "status": "open"}]}
    if stage == "analyze":
        c = read("collection.json")
        return {"question": c["question"], "answer": f'{len(c["observations"])} observations were recorded.',
                "source": "collection.json", "unresolved_issues": [item["id"] for item in c["backlog"]]}
    if stage == "fix":
        c = read("collection.json")
        return {"processed_ids": [item["id"] for item in c["backlog"]],
                "disposition": "recorded_for_follow_up", "analysis_preserved": "analysis.json"}
    if stage == "finalize":
        a, f = read("analysis.json"), read("fixes.json")
        return {"question": a["question"], "answer": a["answer"],
                "backlog_disposition": f["disposition"], "source": "analysis.json"}
    raise ValueError("unknown stage")


def main():
    if len(sys.argv) != 2:
        raise ValueError("expected role name")
    role = sys.argv[1]
    packet = read(os.environ["SIMON_PACKET"])
    gate, stage = packet["gate"], packet["stage"]
    if gate == "unblocker" and role == "unblocker":
        emit(packet, "blocked", "This fixture cannot resolve the reported blocker automatically.")
        return
    path = {"investigate": "investigation.json", "supervise": "collection.json",
            "analyze": "analysis.json", "fix": "fixes.json", "finalize": "final.json"}.get(stage)
    if path is None:
        raise ValueError("unsupported stage")
    if gate == "worker":
        expected = {"investigate": "investigator", "supervise": "supervisor", "analyze": "analyzer", "fix": "fixer", "finalize": "finalizer"}[stage]
        if role != expected:
            raise ValueError("role does not match stage")
        try:
            value = result(stage, packet)
        except PermissionError as error:
            emit(packet, "blocked", str(error))
            return
        Path(path).write_text(json.dumps(value, indent=2) + "\n")
        subprocess.run(["git", "add", "--", path], check=True)
        subprocess.run(["git", "-c", "user.name=Role Handoff Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", f"Record {stage}"], check=True)
        emit(packet, "complete", f"{role} recorded {path}.", candidate=git("rev-parse", "HEAD"))
    elif gate == "reviewer" and role == "reviewer":
        head = git("rev-parse", "HEAD")
        try:
            actual = read(path)
            expected = result(stage, packet)
            good = actual == expected
        except (OSError, ValueError, KeyError, PermissionError):
            good = False
        emit(packet, "pass" if good else "changes_required", f"Checked {stage} evidence and role output.", inspected=head)
    else:
        raise ValueError("role does not match gate")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"role handoff failed: {error}", file=sys.stderr)
        sys.exit(1)
