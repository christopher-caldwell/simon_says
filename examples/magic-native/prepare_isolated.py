#!/usr/bin/env python3
"""Copy a local collection_plan simulator into a new isolated runtime root."""
import argparse
import json
from pathlib import Path
import shutil


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("collection_repo", type=Path)
    parser.add_argument("new_runtime_root", type=Path)
    args = parser.parse_args()
    source = args.collection_repo.resolve(strict=True)
    target = args.new_runtime_root.absolute()
    target.mkdir(parents=False, exist_ok=False)
    target = target.resolve(strict=True)
    (target / "repo/sim").mkdir(parents=True)
    shutil.copytree(source / "sim/manabrew", target / "repo/sim/manabrew", ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
    shutil.copy2(source / "sim/playtest", target / "repo/sim/playtest")
    for deck, checkpoint in (("angels", 2), ("miirym", 2), ("azula", 1), ("aang", 1)):
        relative = Path("decks") / deck / f"checkpoint_{checkpoint}"
        (target / "repo" / relative).mkdir(parents=True)
        for name in ("deck_list.csv", "GUIDE.md"):
            shutil.copy2(source / relative / name, target / "repo" / relative / name)
    receipt = json.loads((source / "sim/.local/manabrew/runtime.json").read_text())
    objects = target / "store/object"
    objects.mkdir(parents=True)
    for key, filename in (("binary", "engine"), ("archive", "cardset.rkyv")):
        shutil.copy2(receipt[key], objects / filename)
        receipt[key] = str(objects / filename)
    (target / "store/runtime.json").write_text(json.dumps(receipt) + "\n")
    print(target)


if __name__ == "__main__":
    main()
