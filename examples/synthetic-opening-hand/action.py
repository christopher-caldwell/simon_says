#!/usr/bin/env python3
"""Synthetic opening-hand fixture; no real Magic rules are modeled."""
import json
import os
from pathlib import Path
import random
import subprocess
import sys


def git(*args):
    return subprocess.check_output(["git", *args], text=True).strip()


def reply(packet, outcome, report, candidate=None, inspected=None):
    print(json.dumps({"action_id": packet["action_id"], "outcome": outcome,
                      "report": report, "candidate_commit": candidate,
                      "inspected_commit": inspected}))


def load(path):
    return json.loads(Path(path).read_text())


def samples(config, revision):
    size, lands, hand, trials = (config[k] for k in ("deck_size", "lands", "hand_size", "trials"))
    if any(type(n) is not int for n in (size, lands, hand, trials)) or size != 60 or hand != 7 or not 0 <= lands <= size or not 1 <= trials <= 10000:
        raise ValueError("invalid synthetic experiment configuration")
    generator = random.Random(config["seed"])
    histogram = [0] * (hand + 1)
    deck = [True] * lands + [False] * (size - lands)
    for _ in range(trials):
        histogram[sum(generator.sample(deck, hand))] += 1
    return {"trials": trials, "histogram": histogram, "revision": revision}


def analysis(sample):
    total = sum(sample["histogram"][2:5])
    return {"trials": sample["trials"], "two_to_four_lands": total, "fraction": total / sample["trials"]}


def main():
    packet = load(os.environ["SIMON_PACKET"])
    gate, stage = packet["gate"], packet["stage"]
    config = load("experiment.json")
    exercise = config.get("exercise", "normal")
    feedback = packet.get("feedback", [])
    if gate == "unblocker":
        reply(packet, "retry", "Fixture unblocker permits one retry.")
        return
    if gate == "worker" and stage == "simulate":
        if exercise == "always_block" or (exercise == "block_once" and not any("permits one retry" in item for item in feedback)):
            reply(packet, "blocked", "Synthetic fixture requested a recoverable block.")
            return
        revision = 2 if any("revision 2" in item for item in feedback) else 1
        result, filename = samples(config, revision), "samples.json"
    elif gate == "worker" and stage == "analyze":
        result, filename = analysis(load("samples.json")), "analysis.json"
    elif gate == "reviewer":
        head = git("rev-parse", "HEAD")
        if stage == "simulate":
            actual = load("samples.json")
            if actual != samples(config, actual["revision"]):
                reply(packet, "changes_required", "The simulation does not match its seed and inputs.", inspected=head)
            elif exercise == "review_once" and actual["revision"] < 2:
                reply(packet, "changes_required", "Please produce revision 2 as the review exercise.", inspected=head)
            else:
                reply(packet, "pass", "Verified the synthetic simulation.", inspected=head)
        else:
            actual = load("analysis.json")
            expected = analysis(load("samples.json"))
            reply(packet, "pass" if actual == expected else "changes_required", "Checked the analysis against the sample histogram.", inspected=head)
        return
    else:
        raise ValueError(f"unsupported action {stage}:{gate}")
    Path(filename).write_text(json.dumps(result, indent=2) + "\n")
    subprocess.run(["git", "add", "--", filename], check=True)
    subprocess.run(["git", "-c", "user.name=Simon Says Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-m", f"Create {filename}"], stdout=subprocess.DEVNULL, check=True)
    reply(packet, "complete", f"Created {filename}.", candidate=git("rev-parse", "HEAD"))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"synthetic fixture failed: {error}", file=sys.stderr)
        sys.exit(1)
