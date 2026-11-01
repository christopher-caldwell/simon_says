#!/usr/bin/env python3
"""Prepare an isolated Git project with frozen adaptations of real Magic roles."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib
sys.dont_write_bytecode = True
from runtime_source import capture, digest_manifest

CONTEXT = ("Can this native runtime produce one verified eligible diagnostic game, "
           "and what does that observation not establish?")
OVERRIDES = {
    "investigator": "Define the frozen diagnostic and preserve the exact original question. No games, dispatch, scheduler, deck changes or purchases.",
    "supervisor": "Run the one explicitly authorized isolated diagnostic through native_tool.py, verify structure and capture issue inventory. No performance interpretation, retries, repair, dispatch, scheduler or extra games.",
    "analyzer": "Independently inspect the original question and verified evidence in a fresh provider invocation. Produce a bounded draft; no games, repair, selected-list changes, publication, finalization or data removal.",
    "fixer": "Configured for a separate explicitly requested repair scope only. This diagnostic never routes to Fixer, authorizes no repair and does not change the engine.",
}
GATES = {
    "investigator": ("## Gate 1:", "## Gate 2:"),
    "supervisor": ("## Gate 2:", "## Gate 4:"),
    "analyzer": ("## Gate 4:", "## Separate repair gate"),
    "fixer": ("## Separate repair gate", None),
}


def provenance(path, root, data):
    return f"{path.relative_to(root)} sha256={hashlib.sha256(data).hexdigest()}"


def git(repo, *args):
    subprocess.run(["git", "-C", str(repo), *args], check=True, stdout=subprocess.DEVNULL)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("collection_repo", type=Path, help="read-only source of actual role profiles and guidance")
    ap.add_argument("isolated_runtime_root", type=Path, help="runtime prepared by examples/magic-native/prepare_isolated.py")
    ap.add_argument("new_project", type=Path, help="must not exist")
    args = ap.parse_args()
    source = args.collection_repo.resolve(strict=True)
    source_head = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
    runtime = args.isolated_runtime_root.resolve(strict=True)
    runtime_manifest = digest_manifest(capture(runtime))
    requested = args.new_project.absolute()
    parent = requested.parent.resolve(strict=True)
    target = parent / requested.name
    if target.exists() or target.is_symlink():
        raise ValueError("new project already exists")
    if target.is_relative_to(source) or target.is_relative_to(runtime):
        raise ValueError("new project must be outside the source repository and runtime")
    target.mkdir()
    target = target.resolve(strict=True)
    for item in Path(__file__).resolve().parent.iterdir():
        if item.is_file():
            shutil.copy2(item, target / item.name)
    for deck, checkpoint in (("angels", 2), ("miirym", 2), ("azula", 1), ("aang", 1)):
        rel = Path("decks") / deck / f"checkpoint_{checkpoint}"
        (target / rel).mkdir(parents=True)
        for name in ("deck_list.csv", "GUIDE.md"):
            shutil.copy2(runtime / "repo" / rel / name, target / rel / name)
    role_dir = target / "profiles"
    role_dir.mkdir()
    source_intent = source / "guides/project_intent.md"
    source_gates = source / ".agents/skills/ai_playtest/references/roles-and-gates.md"
    gates_bytes = source_gates.read_bytes()
    intent_bytes = source_intent.read_bytes()
    captured = {source_gates: gates_bytes, source_intent: intent_bytes}
    gates = gates_bytes.decode("utf-8")
    project_intent = intent_bytes.decode("utf-8")
    workflow_path = target / "workflow.toml"
    workflow = workflow_path.read_text()
    for role, (start, end) in GATES.items():
        source_role = source / ".codex/agents" / f"{role}.toml"
        role_bytes = source_role.read_bytes()
        captured[source_role] = role_bytes
        raw = role_bytes.decode("utf-8")
        config = tomllib.loads(raw)
        section = gates[gates.index(start):gates.index(end) if end else len(gates)]
        # These are separate diagnostic adaptations; source excerpts are recorded
        # as exact provenance, not competing live dispatch/scheduling instructions.
        adapted = ("# Isolated diagnostic role: " + role + "\n\n"
                   "The following task-specific responsibilities are operative. The source excerpts below are frozen historical authority/evidence; "
                   "their instructions to spawn chats, schedule checks, invoke production workflows or finalize/retire raw data are not operative for this explicitly bounded Simon Says diagnostic. "
                   "Never read the live source checkout. The engine creates fresh provider invocations and owns handoffs. "
                   "Checkpoint commits are isolated workflow evidence, not publication, merging or runtime installation.\n\n"
                   + OVERRIDES[role] + "\n\n# Exact source provenance\nsource HEAD=" + source_head + " (working-tree files may include uncommitted changes)\n"
                   + "\n".join(provenance(path, source, data) for path, data in ((source_role, role_bytes), (source_intent, intent_bytes), (source_gates, gates_bytes)))
                   + "\n\n# Source role TOML (historical evidence; do not self-dispatch)\n```toml\n" + raw
                   + "\n```\n\n# Source project intent (exact text)\n" + project_intent
                   + "\n\n# Applicable source gate excerpt (historical evidence; diagnostic overrides apply)\n" + section)
        (role_dir / f"{role}.md").write_text(adapted)
        marker = {"investigator": ("gpt-6.1-terra", "xhigh"), "supervisor": ("gpt-6-luna", "high"), "analyzer": ("gpt-6.1-sol", "high"), "fixer": ("gpt-6.1-terra", "high")}[role]
        role_section = f"[roles.{role}]"
        a = workflow.index(role_section)
        b = workflow.find("\n[", a + len(role_section))
        b = len(workflow) if b < 0 else b
        chunk = workflow[a:b]
        chunk = chunk.replace(f'model = "{marker[0]}"', "model = " + json.dumps(config["model"]))
        chunk = chunk.replace(f'effort = "{marker[1]}"', "effort = " + json.dumps(config["model_reasoning_effort"]))
        workflow = workflow[:a] + chunk + workflow[b:]
    workflow_path.write_text(workflow)
    (target / "runtime_source_manifest.json").write_text(json.dumps(runtime_manifest, indent=2, sort_keys=True) + "\n")
    config_path = target / "diagnostic.json"
    config = json.loads(config_path.read_text())
    config["question"] = CONTEXT
    config_path.write_text(json.dumps(config, indent=2) + "\n")
    (target / "original-task.txt").write_text(CONTEXT)
    if digest_manifest(capture(runtime)) != runtime_manifest:
        raise RuntimeError("native runtime Python source or receipt changed during preparation; inspect this incomplete project and prepare a new one")
    if subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip() != source_head:
        raise RuntimeError("source HEAD changed during preparation; inspect this incomplete project and prepare a new one")
    for source_path, original in captured.items():
        if source_path.read_bytes() != original:
            raise RuntimeError(f"source authority changed during preparation: {source_path}; inspect this incomplete project and prepare a new one")
    git(target, "init", "-q")
    git(target, "config", "user.name", "Simon Says Diagnostic")
    git(target, "config", "user.email", "simon-diagnostic@example.invalid")
    git(target, "add", ".")
    git(target, "commit", "-qm", "Prepare isolated diagnostic inputs")
    print(json.dumps({"project": str(target), "context": str(target / "original-task.txt"),
                      "source_head": source_head, "runtime": str(runtime), "note": "Isolated diagnostic only; no production finalization or retirement."}))


if __name__ == "__main__":
    main()
