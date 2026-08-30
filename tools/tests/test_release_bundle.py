"""Contracts for the recoverable, copy-only workbook release bundle."""

import ast
import hashlib
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


TOOLS = Path(__file__).resolve().parents[1]
ROOT = TOOLS.parent
sys.path.insert(0, str(TOOLS))

import prepare_release_bundle as release_bundle


SOURCE_COMMIT = "a" * 40
GATE_RESULTS = [
    {"command": command, "output": f"OK: fixture gate {number}"}
    for number, command in enumerate(release_bundle.GATE_COMMANDS, start=1)
]


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def git_blob_sha1(data):
    header = f"blob {len(data)}\0".encode("ascii")
    return hashlib.sha1(header + data, usedforsecurity=False).hexdigest()


class ReleaseBundleTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.mkdtemp(prefix="ozzit-release-bundle-")
        self.directory = Path(self.temporary)
        self.root = self.directory / "repo"
        self.root.mkdir()
        self.workbook = self.root / "ozzit.xlsx"
        self.workbook.write_bytes(b"synthetic workbook fixture\n")
        self.manifest = self.root / "release" / "workbook-base.json"
        self.manifest.parent.mkdir()
        document = {
            "git_blob_sha1": git_blob_sha1(self.workbook.read_bytes()),
            "last_workbook_commit": "b" * 40,
            "path": "ozzit.xlsx",
            "schema": 1,
            "sha256": sha256(self.workbook.read_bytes()),
            "size": self.workbook.stat().st_size,
        }
        self.manifest.write_text(
            json.dumps(document, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def create(self, output):
        with mock.patch.object(
            release_bundle,
            "run_semantic_gates",
            return_value=GATE_RESULTS,
        ):
            return release_bundle.create_bundle(
                root=self.root,
                workbook=self.workbook,
                base_manifest=self.manifest,
                output=output,
                version="3.2.0",
                source_commit=SOURCE_COMMIT,
            )

    def test_tracked_base_manifest_matches_the_current_workbook(self):
        base = release_bundle.verify_base_workbook(
            ROOT / "ozzit.xlsx",
            ROOT / "release" / "workbook-base.json",
        )

        self.assertEqual(base["sha256"], sha256((ROOT / "ozzit.xlsx").read_bytes()))
        self.assertEqual(base["size"], (ROOT / "ozzit.xlsx").stat().st_size)
        release_bundle.verify_repository_base(ROOT, base)

    def test_create_writes_only_the_three_reviewed_assets(self):
        output = self.directory / "bundle"

        provenance = self.create(output)

        self.assertEqual(
            sorted(path.name for path in output.iterdir()),
            ["SHA256SUMS", "ozzit.xlsx", "provenance.json"],
        )
        self.assertEqual(provenance["version"], "3.2.0")
        self.assertEqual(provenance["source"]["commit"], SOURCE_COMMIT)
        self.assertEqual(provenance["verification"]["static_gates"], GATE_RESULTS)
        self.assertEqual(
            provenance["assets"],
            ["ozzit.xlsx", "provenance.json", "SHA256SUMS"],
        )
        self.assertEqual((output / "ozzit.xlsx").read_bytes(), self.workbook.read_bytes())
        self.assertEqual(
            release_bundle.verify_bundle(output, root=self.root, check_semantics=False),
            provenance,
        )

    def test_checksum_file_covers_workbook_and_provenance_without_a_cycle(self):
        output = self.directory / "bundle"
        self.create(output)

        checksums = release_bundle.parse_checksums(output / "SHA256SUMS")

        self.assertEqual(sorted(checksums), ["ozzit.xlsx", "provenance.json"])
        for name, expected in checksums.items():
            self.assertEqual(expected, sha256((output / name).read_bytes()))

    def test_two_staged_builds_are_byte_identical(self):
        first = self.directory / "first"
        second = self.directory / "second"
        self.create(first)
        self.create(second)

        for name in ("ozzit.xlsx", "provenance.json", "SHA256SUMS"):
            with self.subTest(name=name):
                self.assertEqual((first / name).read_bytes(), (second / name).read_bytes())

    def test_existing_output_is_never_replaced(self):
        output = self.directory / "bundle"
        output.mkdir()
        sentinel = output / "keep.txt"
        sentinel.write_text("prior verified bundle", encoding="utf-8")

        with self.assertRaises(FileExistsError):
            self.create(output)

        self.assertEqual(sentinel.read_text(encoding="utf-8"), "prior verified bundle")
        self.assertEqual(list(output.iterdir()), [sentinel])

    def test_gate_failure_preserves_the_source_and_publishes_nothing(self):
        output = self.directory / "bundle"
        before = self.workbook.read_bytes()

        with mock.patch.object(
            release_bundle,
            "run_semantic_gates",
            side_effect=RuntimeError("gate failed"),
        ), self.assertRaisesRegex(RuntimeError, "gate failed"):
            release_bundle.create_bundle(
                root=self.root,
                workbook=self.workbook,
                base_manifest=self.manifest,
                output=output,
                version="3.2.0",
                source_commit=SOURCE_COMMIT,
            )

        self.assertEqual(self.workbook.read_bytes(), before)
        self.assertFalse(output.exists())
        self.assertEqual(
            [path for path in self.directory.iterdir() if path.name.startswith(".bundle-")],
            [],
        )

    def test_structural_failure_cleans_staging_and_publishes_nothing(self):
        output = self.directory / "bundle"

        with mock.patch.object(
            release_bundle,
            "run_semantic_gates",
            return_value=GATE_RESULTS,
        ), mock.patch.object(
            release_bundle,
            "verify_bundle",
            side_effect=ValueError("structural verification failed"),
        ), self.assertRaisesRegex(ValueError, "structural verification failed"):
            release_bundle.create_bundle(
                root=self.root,
                workbook=self.workbook,
                base_manifest=self.manifest,
                output=output,
                version="3.2.0",
                source_commit=SOURCE_COMMIT,
            )

        self.assertFalse(output.exists())
        self.assertEqual(
            [path for path in self.directory.iterdir() if path.name.startswith(".bundle-")],
            [],
        )

    def test_output_inside_the_source_repository_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "outside the source repository"):
            self.create(self.root / "bundle")

    def test_base_mismatch_fails_before_staging(self):
        self.workbook.write_bytes(b"changed workbook\n")

        with self.assertRaisesRegex(ValueError, "base workbook"):
            self.create(self.directory / "bundle")

    def test_verifier_rejects_tampering_and_extra_assets(self):
        output = self.directory / "bundle"
        self.create(output)

        (output / "extra.txt").write_text("not reviewed", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "inventory"):
            release_bundle.verify_bundle(output, root=self.root, check_semantics=False)
        (output / "extra.txt").unlink()

        (output / "ozzit.xlsx").write_bytes(b"tampered")
        with self.assertRaisesRegex(ValueError, "checksum"):
            release_bundle.verify_bundle(output, root=self.root, check_semantics=False)

    def test_independent_semantic_results_must_match_provenance(self):
        output = self.directory / "bundle"
        self.create(output)

        with mock.patch.object(
            release_bundle,
            "run_semantic_gates",
            return_value=GATE_RESULTS,
        ):
            release_bundle.verify_bundle(output, root=self.root, check_semantics=True)

        with mock.patch.object(
            release_bundle,
            "run_semantic_gates",
            return_value=[{"command": "same", "output": "different"}],
        ), self.assertRaisesRegex(ValueError, "semantic gate evidence"):
            release_bundle.verify_bundle(output, root=self.root, check_semantics=True)

    def test_verifier_rejects_falsified_verification_claims(self):
        output = self.directory / "bundle"
        self.create(output)
        provenance_path = output / "provenance.json"
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        provenance["verification"]["native_gates"]["included"] = True
        provenance_path.write_bytes(release_bundle.canonical_json(provenance))
        checksums = {
            name: sha256((output / name).read_bytes())
            for name in ("ozzit.xlsx", "provenance.json")
        }
        (output / "SHA256SUMS").write_text(
            "".join(f"{checksums[name]}  {name}\n" for name in sorted(checksums)),
            encoding="ascii",
            newline="",
        )

        with self.assertRaisesRegex(ValueError, "verification claims"):
            release_bundle.verify_bundle(output, root=self.root, check_semantics=False)

    def test_verifier_rejects_extra_top_level_provenance_claims(self):
        output = self.directory / "bundle"
        self.create(output)
        provenance_path = output / "provenance.json"
        provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
        provenance["independently_verified"] = True
        provenance_path.write_bytes(release_bundle.canonical_json(provenance))
        checksums = {
            name: sha256((output / name).read_bytes())
            for name in ("ozzit.xlsx", "provenance.json")
        }
        (output / "SHA256SUMS").write_text(
            "".join(f"{checksums[name]}  {name}\n" for name in sorted(checksums)),
            encoding="ascii",
            newline="",
        )

        with self.assertRaisesRegex(ValueError, "top-level schema"):
            release_bundle.verify_bundle(output, root=self.root, check_semantics=False)

    def test_version_and_commit_are_closed_canonical_values(self):
        for version in ("v3.2.0", "03.2.0", "3.2", "3.2.0; touch pwned"):
            with self.subTest(version=version), self.assertRaises(ValueError):
                release_bundle.validate_version(version)
        for commit in ("main", "a" * 39, "A" * 40, "a" * 40 + ";"):
            with self.subTest(commit=commit), self.assertRaises(ValueError):
                release_bundle.validate_source_commit(commit)

    def test_subprocesses_always_receive_argument_lists_without_a_shell(self):
        source = (TOOLS / "prepare_release_bundle.py").read_text(encoding="utf-8")
        tree = ast.parse(source)
        calls = [
            node
            for node in ast.walk(tree)
            if isinstance(node, ast.Call)
            and isinstance(node.func, ast.Attribute)
            and isinstance(node.func.value, ast.Name)
            and node.func.value.id == "subprocess"
            and node.func.attr == "run"
        ]

        self.assertGreaterEqual(len(calls), 2)
        for call in calls:
            with self.subTest(line=call.lineno):
                self.assertIsInstance(call.args[0], ast.List)
                shell = next(
                    (keyword.value for keyword in call.keywords if keyword.arg == "shell"),
                    None,
                )
                self.assertTrue(
                    shell is None
                    or (isinstance(shell, ast.Constant) and shell.value is False)
                )

    def test_gate_evidence_is_independent_of_how_the_workbook_path_is_written(self):
        """create records in-repo evidence and verify re-runs against the staged copy.

        The gates echo the workbook path they are handed, so evidence recorded from
        one spelling of that path has to match evidence re-run from another. The
        bundle is always staged outside the repository, so the two spellings are
        never identical in practice.
        """
        recorded = []

        def echo_arguments(argv, **_):
            recorded.append(argv)
            return mock.Mock(returncode=0, stdout="OK: " + " ".join(argv[4:]) + "\n")

        detoured = self.root / ".." / self.root.name / "ozzit.xlsx"
        self.assertIn("..", str(detoured))

        with mock.patch.object(release_bundle.subprocess, "run", echo_arguments):
            direct = release_bundle.run_semantic_gates(self.root, self.workbook)
            indirect = release_bundle.run_semantic_gates(self.root, detoured)

        self.assertEqual(direct, indirect)
        self.assertNotIn("..", json.dumps(direct))
        for argv in recorded:
            with self.subTest(argv=argv):
                for argument in argv[4:]:
                    self.assertTrue(Path(argument).is_absolute())


if __name__ == "__main__":
    unittest.main()
