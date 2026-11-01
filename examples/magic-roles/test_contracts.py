#!/usr/bin/env python3
"""Real-Git tests of the diagnostic handoff; no models or games are run."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

FIXTURE = Path(__file__).resolve().parent


class Contracts(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        for name in ("review.py", "unblock.py", "native_tool.py", "runtime_source.py", "ps_compat.py", "diagnostic.json"):
            shutil.copy2(FIXTURE / name, self.repo / name)
        subprocess.run(["git", "init", "-q", str(self.repo)], check=True)
        self.git("config", "user.name", "Test")
        self.git("config", "user.email", "test@example.invalid")
        self.packet = self.root / "packet.json"
        self.scratch = self.root / "scratch"
        self.scratch.mkdir()
        self.context = "Exact original question."

    def tearDown(self):
        self.tmp.cleanup()

    def git(self, *args):
        return subprocess.check_output(["git", "-C", str(self.repo), *args], text=True).strip()

    def commit(self):
        self.git("add", ".")
        self.git("commit", "-qm", "fixture")
        return self.git("rev-parse", "HEAD")

    def invoke(self, script, packet, *args):
        self.packet.write_text(json.dumps(packet))
        env = dict(os.environ, SIMON_PACKET=str(self.packet), SIMON_SCRATCH=str(self.scratch), PYTHONDONTWRITEBYTECODE="1")
        return subprocess.run([sys.executable, "-B", str(self.repo / script), *args], cwd=self.repo, env=env, capture_output=True, text=True)

    def review_packet(self, stage, head, question=None):
        return {"action_id": "a-1", "stage": stage, "gate": "reviewer", "candidate_commit": head,
                "run_context": self.context if question is None else question}

    def test_design_binds_original_question_and_config(self):
        config = (self.repo / "diagnostic.json").read_bytes()
        design = {"original_question": self.context, "config_sha256": hashlib.sha256(config).hexdigest(), "planned_games": 1,
                  "hypothesis": "A game can be verified.", "decision": "Run one diagnostic.", "limitations": "Not a deck comparison."}
        (self.repo / "design.json").write_text(json.dumps(design))
        head = self.commit()
        result = self.invoke("review.py", self.review_packet("investigate", head))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)["inspected_commit"], head)
        mismatch = self.invoke("review.py", self.review_packet("investigate", head, "Different question"))
        self.assertEqual(mismatch.returncode, 0, mismatch.stderr)
        self.assertEqual(json.loads(mismatch.stdout)["outcome"], "changes_required")
        self.assertEqual(self.git("status", "--porcelain"), "")

    def test_analysis_must_preserve_question_count_and_issue_inventory(self):
        issues = [{"kind": "warning", "details": "unresolved"}]
        (self.repo / "collection.json").write_text(json.dumps({"eligible": 1, "issues": issues}))
        (self.repo / "analysis.json").write_text(json.dumps({"original_question": self.context, "verified_games": 1, "issues": issues, "answer": "One diagnostic.", "limitations": "Not a deck comparison."}))
        head = self.commit()
        result = self.invoke("review.py", self.review_packet("analyze", head))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)["outcome"], "pass")
        changed = self.invoke("review.py", self.review_packet("analyze", head, "Different question"))
        self.assertEqual(changed.returncode, 0, changed.stderr)
        self.assertEqual(json.loads(changed.stdout)["outcome"], "changes_required")

    def test_unverifiable_collection_blocks_without_retry(self):
        (self.repo / "collection.json").write_text(json.dumps({"original_question": self.context, "store": "/does/not/exist", "run": "one"}))
        head = self.commit()
        result = self.invoke("review.py", self.review_packet("supervise", head))
        self.assertEqual(result.returncode, 0, result.stderr)
        response = json.loads(result.stdout)
        self.assertEqual(response["outcome"], "blocked")
        self.assertIn("no retry", response["report"])

    def test_prepare_freezes_source_profiles_and_configured_models_without_source_mutation(self):
        import tomllib
        source = self.root / "magic-source"
        source.mkdir()
        (source / ".codex/agents").mkdir(parents=True)
        (source / ".agents/skills/ai_playtest/references").mkdir(parents=True)
        (source / "guides").mkdir()
        (source / "guides/project_intent.md").write_text("Source intent for diagnostic.\n")
        (source / ".agents/skills/ai_playtest/references/roles-and-gates.md").write_text("## Gate 1:\nDesign.\n## Gate 2:\nSupervise.\n## Gate 3:\nHandoff.\n## Gate 4:\nAnalyze.\n## Gate 5:\nApproval.\n## Separate repair gate\nSeparate repair.\n")
        models = {"investigator": ("model-investigator", "xhigh"), "supervisor": ("model-supervisor", "high"), "analyzer": ("model-analyzer", "high"), "fixer": ("model-fixer", "high")}
        for role, (model, effort) in models.items():
            (source / f".codex/agents/{role}.toml").write_text(f'name = "{role}"\nmodel = "{model}"\nmodel_reasoning_effort = "{effort}"\ndeveloper_instructions = "fixture"\n')
        subprocess.run(["git", "init", "-q", str(source)], check=True)
        subprocess.run(["git", "-C", str(source), "config", "user.name", "Test"], check=True)
        subprocess.run(["git", "-C", str(source), "config", "user.email", "test@example.invalid"], check=True)
        subprocess.run(["git", "-C", str(source), "add", "."], check=True)
        subprocess.run(["git", "-C", str(source), "commit", "-qm", "source"], check=True)
        runtime = self.root / "runtime"
        (runtime / "repo/sim/manabrew").mkdir(parents=True)
        (runtime / "repo/sim/playtest").write_text("fixture launcher\n")
        (runtime / "repo/sim/manabrew/__init__.py").write_text("fixture package\n")
        (runtime / "store").mkdir()
        (runtime / "store/runtime.json").write_text("{}\n")
        for deck, cp in (("angels", 2), ("miirym", 2), ("azula", 1), ("aang", 1)):
            path = runtime / "repo/decks" / deck / f"checkpoint_{cp}"
            path.mkdir(parents=True)
            for name in ("deck_list.csv", "GUIDE.md"):
                (path / name).write_text("fixture\n")
        project = self.root / "project"
        process = subprocess.run([sys.executable, "-B", str(FIXTURE / "prepare.py"), str(source), str(runtime), str(project)], capture_output=True, text=True)
        self.assertEqual(process.returncode, 0, process.stderr)
        config = tomllib.loads((project / "workflow.toml").read_text())
        for role, (model, effort) in models.items():
            self.assertEqual((config["roles"][role]["model"], config["roles"][role]["effort"]), (model, effort))
            profile = (project / "profiles" / f"{role}.md").read_text()
            raw = (source / f".codex/agents/{role}.toml").read_bytes()
            self.assertIn(hashlib.sha256(raw).hexdigest(), profile)
            self.assertIn(raw.decode(), profile)
        original = subprocess.check_output(["git", "-C", str(source), "status", "--porcelain"], text=True)
        self.assertEqual(original, "")
        self.assertEqual(subprocess.check_output(["git", "-C", str(project), "status", "--porcelain"], text=True), "")

    def test_runtime_manifest_rejects_changed_added_and_symlinked_sources(self):
        from runtime_source import capture, digest_manifest, verify
        runtime = self.root / "native"
        package = runtime / "repo/sim/manabrew"
        package.mkdir(parents=True)
        (runtime / "repo/sim/playtest").write_text("launcher")
        (package / "__init__.py").write_text("source")
        (runtime / "store").mkdir()
        (runtime / "store/runtime.json").write_text("{}")
        manifest = digest_manifest(capture(runtime))
        self.assertEqual(verify(runtime, manifest)["repo/sim/playtest"], b"launcher")
        (package / "__init__.py").write_text("changed")
        with self.assertRaises(ValueError): verify(runtime, manifest)
        (package / "__init__.py").write_text("source")
        (package / "unexpected.py").write_text("untracked code")
        with self.assertRaises(ValueError): verify(runtime, manifest)
        (package / "unexpected.py").unlink()
        (package / "linked.py").symlink_to("__init__.py")
        with self.assertRaises(ValueError): verify(runtime, manifest)

    def test_unblocker_does_not_authorize_extra_games(self):
        result = self.invoke("unblock.py", {"action_id": "a-3"})
        self.assertEqual(result.returncode, 0)
        self.assertEqual(json.loads(result.stdout)["outcome"], "blocked")

    def test_collection_refuses_to_run_without_frozen_design(self):
        result = self.invoke("native_tool.py", {"action_id": "a-0", "stage": "supervise", "gate": "worker", "run_context": self.context}, "collect")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.repo / "collection.json").exists())


if __name__ == "__main__":
    unittest.main()
