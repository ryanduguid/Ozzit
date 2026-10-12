"""Rejection paths and command-line flow of the copy-only release bundle tool.

Git and the gate scripts are always replaced with fakes here: nothing reads or
changes the real repository history.
"""

import contextlib
import hashlib
import io
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))

import prepare_release_bundle as release_bundle  # noqa: E402

HEAD = "a" * 40
LAST_WORKBOOK_COMMIT = "b" * 40
GATE_RESULTS = [
    {"command": command, "output": f"OK: fixture gate {number}"}
    for number, command in enumerate(release_bundle.GATE_COMMANDS, start=1)
]


def set_claim(*keys, value):
    def change(provenance):
        target = provenance
        for key in keys[:-1]:
            target = target[key]
        target[keys[-1]] = value

    return change


PROVENANCE_DEFECTS = (
    (set_claim("schema", value=2), "provenance.json identifies an unsupported bundle"),
    (set_claim("schema", value=True), "provenance.json identifies an unsupported bundle"),
    (set_claim("project", value="Other"), "provenance.json identifies an unsupported bundle"),
    (set_claim("assets", value=["ozzit.xlsx"]), "provenance asset inventory is invalid"),
    (set_claim("version", value=3), "provenance version is invalid"),
    (set_claim("version", value="v3.2.0"), "version must be canonical X.Y.Z"),
    (set_claim("source", value="main"), "provenance source is invalid"),
    (set_claim("source", "extra", value=1), "provenance source is invalid"),
    (set_claim("source", "commit", value=7), "provenance source commit is invalid"),
    (set_claim("source", "commit", value="main"), "source commit must be a full 40-character"),
    (
        set_claim("source", "workbook_path", value="other.xlsx"),
        "provenance workbook path is invalid",
    ),
    (
        set_claim("workbook", "size", value=1),
        "provenance workbook evidence does not match the base workbook",
    ),
    # 1 == True, so the evidence compares equal and only the type check catches it.
    (
        set_claim("workbook", "copy_only", value=1),
        "provenance workbook copy-only claim is invalid",
    ),
    (set_claim("verification", value=[]), "provenance verification evidence is invalid"),
    (
        set_claim("verification", "static_gates", value="all passed"),
        "provenance static gate evidence is invalid",
    ),
    (
        set_claim("verification", "static_gates", value=[["command", "output"]]),
        "provenance static gate evidence is invalid",
    ),
    (
        set_claim("verification", "static_gates", value=[{"command": "x"}]),
        "provenance static gate evidence is invalid",
    ),
    (
        set_claim("verification", "static_gates", value=[{"command": 1, "output": ""}]),
        "provenance static gate evidence is invalid",
    ),
    (
        set_claim("verification", "static_gates", value=[{"command": "", "output": 1}]),
        "provenance static gate evidence is invalid",
    ),
    (
        set_claim("verification", "static_gates", value=list(reversed(GATE_RESULTS))),
        "provenance static gate sequence is invalid",
    ),
    (
        set_claim("verification", "extra", value=True),
        "provenance verification claims are invalid",
    ),
    # 0 == False, so the claim compares equal and only the type check catches it.
    (
        set_claim("verification", "native_gates", "included", value=0),
        "provenance native gate claims must be Boolean",
    ),
)


class ReleaseFixture(unittest.TestCase):
    """A synthetic checkout with a workbook, its base manifest and a fake Git."""

    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="ozzit-release-paths-")).resolve()
        self.root = self.directory / "repo"
        self.root.mkdir()
        self.workbook = self.root / "ozzit.xlsx"
        self.workbook.write_bytes(b"synthetic workbook fixture\n")
        self.manifest = self.root / "release" / "workbook-base.json"
        self.manifest.parent.mkdir()
        self.base = {
            "git_blob_sha1": release_bundle.git_blob_sha1(self.workbook),
            "last_workbook_commit": LAST_WORKBOOK_COMMIT,
            "path": "ozzit.xlsx",
            "schema": 1,
            "sha256": hashlib.sha256(self.workbook.read_bytes()).hexdigest(),
            "size": self.workbook.stat().st_size,
        }
        self.write_manifest(self.base)
        self.git_answers = {
            "ls-files": "ozzit.xlsx",
            "hash-object": self.base["git_blob_sha1"],
            "log": LAST_WORKBOOK_COMMIT,
            "rev-parse": HEAD,
            "status": "",
        }
        self.git_calls = []

    def tearDown(self):
        shutil.rmtree(self.directory, ignore_errors=True)

    def write_manifest(self, document):
        self.manifest.write_text(json.dumps(document) + "\n", encoding="utf-8")

    def fake_git(self, argv, **kwargs):
        self.assertEqual(argv[0], "git")
        self.assertEqual(kwargs["cwd"], self.root)
        self.assertTrue(kwargs["check"])
        self.git_calls.append(argv[1:])
        return SimpleNamespace(returncode=0, stdout=self.git_answers[argv[1]] + "\n")

    def git(self):
        return mock.patch.object(release_bundle.subprocess, "run", self.fake_git)

    def gates(self, results=GATE_RESULTS):
        return mock.patch.object(release_bundle, "run_semantic_gates", return_value=results)

    def create(self, output):
        with self.gates():
            return release_bundle.create_bundle(
                root=self.root,
                workbook=self.workbook,
                base_manifest=self.manifest,
                output=output,
                version="3.2.0",
                source_commit=HEAD,
            )

    def write_checksums(self, bundle):
        checksums = {
            name: release_bundle.sha256_file(bundle / name)
            for name in release_bundle.CHECKSUMMED_ASSETS
        }
        (bundle / "SHA256SUMS").write_bytes(release_bundle._checksum_bytes(checksums))

    def rewrite_provenance(self, bundle, change):
        path = bundle / "provenance.json"
        provenance = json.loads(path.read_text(encoding="utf-8"))
        change(provenance)
        path.write_bytes(release_bundle.canonical_json(provenance))
        self.write_checksums(bundle)


class JsonAndBaseManifestTests(ReleaseFixture):
    def test_duplicate_members_are_rejected(self):
        self.manifest.write_text('{"schema": 1, "schema": 1}', encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "^duplicate JSON member: schema$"):
            release_bundle.verify_base_workbook(self.workbook, self.manifest)

    def test_unreadable_or_malformed_json_is_reported_by_file_name(self):
        cases = (
            ("malformed", b"{not json"),
            ("not UTF-8", b'{"path": "\xff"}'),
        )
        for label, data in cases:
            with self.subTest(label):
                self.manifest.write_bytes(data)
                with self.assertRaisesRegex(ValueError, r"^invalid JSON in workbook-base\.json: "):
                    release_bundle.verify_base_workbook(self.workbook, self.manifest)

    def test_missing_json_file_is_reported_as_invalid(self):
        with self.assertRaisesRegex(ValueError, r"^invalid JSON in absent\.json: "):
            release_bundle._load_json_object(self.directory / "absent.json")

    def test_json_root_must_be_an_object(self):
        self.manifest.write_text("[1, 2]", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, r"^workbook-base\.json must contain a JSON object"):
            release_bundle.verify_base_workbook(self.workbook, self.manifest)

    def test_workbook_and_manifest_must_be_regular_files(self):
        folder = self.directory / "folder"
        folder.mkdir()
        with self.assertRaisesRegex(ValueError, "^base workbook must be a regular non-symlink"):
            release_bundle.verify_base_workbook(folder, self.manifest)
        with self.assertRaisesRegex(
            ValueError, "^base workbook manifest must be a regular non-symlink file$"
        ):
            release_bundle.verify_base_workbook(self.workbook, folder)

    def test_each_manifest_field_is_checked(self):
        cases = (
            ({"extra": 1}, "has an unexpected schema"),
            ({"schema": 2}, "schema must be 1"),
            ({"schema": True}, "schema must be 1"),
            ({"path": "other.xlsx"}, "path must be ozzit.xlsx"),
            ({"size": 0}, "size must be a positive integer"),
            ({"size": "27"}, "size must be a positive integer"),
            ({"sha256": "A" * 64}, "SHA-256 is invalid"),
            ({"sha256": 7}, "SHA-256 is invalid"),
            ({"git_blob_sha1": "c" * 39}, "Git blob SHA-1 is invalid"),
            ({"git_blob_sha1": None}, "Git blob SHA-1 is invalid"),
            ({"last_workbook_commit": 5}, "last commit is invalid"),
        )
        for change, message in cases:
            with self.subTest(change=change):
                self.write_manifest({**self.base, **change})
                with self.assertRaisesRegex(ValueError, f"^base workbook manifest {message}$"):
                    release_bundle.verify_base_workbook(self.workbook, self.manifest)


class RepositoryBindingTests(ReleaseFixture):
    def test_git_queries_are_read_only_and_bind_the_manifest(self):
        with self.git():
            release_bundle.verify_repository_base(self.root, self.base)
            self.assertEqual(release_bundle.current_commit(self.root), HEAD)
            release_bundle.ensure_clean_checkout(self.root)

        self.assertEqual(
            self.git_calls,
            [
                ["ls-files", "--error-unmatch", "--", "ozzit.xlsx"],
                ["hash-object", "--", "ozzit.xlsx"],
                ["log", "-1", "--format=%H", "--", "ozzit.xlsx"],
                ["rev-parse", "--verify", "HEAD"],
                ["status", "--porcelain", "--untracked-files=all"],
            ],
        )

    def test_mismatched_git_claims_are_rejected(self):
        cases = (
            ("ls-files", "", "must be tracked as ozzit.xlsx"),
            ("hash-object", "d" * 40, "Git blob does not match the manifest"),
            ("log", "e" * 40, "last-change commit does not match Git history"),
        )
        answers = self.git_answers
        for query, answer, message in cases:
            with self.subTest(query=query), self.git():
                self.git_answers = {**answers, query: answer}
                with self.assertRaisesRegex(ValueError, f"^base workbook {message}$"):
                    release_bundle.verify_repository_base(self.root, self.base)

    def test_dirty_checkout_and_bad_head_are_refused(self):
        self.git_answers["status"] = "?? stray.txt"
        self.git_answers["rev-parse"] = "HEAD"
        with self.git():
            with self.assertRaisesRegex(ValueError, "must be clean before staging"):
                release_bundle.ensure_clean_checkout(self.root)
            with self.assertRaisesRegex(ValueError, "full 40-character lowercase SHA"):
                release_bundle.current_commit(self.root)


class SemanticGateFailureTests(ReleaseFixture):
    def test_failing_gate_reports_its_command_code_and_normalised_output(self):
        stdout = f"FAIL: {self.workbook}\r\nsee {self.root / 'src'}\r\n"
        failed = SimpleNamespace(returncode=1, stdout=stdout)
        with mock.patch.object(release_bundle.subprocess, "run", return_value=failed):
            with self.assertRaises(RuntimeError) as caught:
                release_bundle.run_semantic_gates(self.root, self.workbook)
        self.assertEqual(
            str(caught.exception),
            "python tools/verify_workbook.py ozzit.xlsx failed with exit code 1\n"
            "FAIL: ozzit.xlsx\nsee src",
        )

    def test_silent_failing_gate_reports_only_its_exit_code(self):
        failed = SimpleNamespace(returncode=3, stdout="\r\n")
        with mock.patch.object(release_bundle.subprocess, "run", return_value=failed):
            with self.assertRaises(RuntimeError) as caught:
                release_bundle.run_semantic_gates(self.root, self.workbook)
        self.assertEqual(
            str(caught.exception),
            "python tools/verify_workbook.py ozzit.xlsx failed with exit code 3",
        )


class ChecksumFileTests(ReleaseFixture):
    def parse(self, data):
        path = self.directory / "SHA256SUMS"
        path.write_bytes(data)
        return release_bundle.parse_checksums(path)

    def test_unreadable_checksum_files_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "^invalid SHA256SUMS: "):
            release_bundle.parse_checksums(self.directory / "absent")
        with self.assertRaisesRegex(ValueError, "^invalid SHA256SUMS: .*ascii"):
            self.parse("é\n".encode("utf-8"))

    def test_malformed_checksum_files_are_rejected(self):
        digest = "f" * 64
        cases = (
            (f"{digest}  ozzit.xlsx".encode(), "must use canonical LF-terminated lines"),
            (f"{digest}  ozzit.xlsx\r\n".encode(), "must use canonical LF-terminated lines"),
            (f"{digest} ozzit.xlsx\n".encode(), "contains a malformed line"),
            (f"{digest}  ozzit.xlsx\n{digest}  ozzit.xlsx\n".encode(), "repeats ozzit.xlsx"),
            (f"{digest}  ozzit.xlsx\n".encode(), "has an unexpected inventory"),
            (
                f"{digest}  provenance.json\n{digest}  ozzit.xlsx\n".encode(),
                "is not canonically ordered",
            ),
        )
        for data, message in cases:
            with self.subTest(message=message, data=data):
                with self.assertRaisesRegex(ValueError, f"^SHA256SUMS {message}$"):
                    self.parse(data)

    def test_canonical_checksum_file_is_accepted(self):
        digest = "f" * 64
        self.assertEqual(
            self.parse(f"{digest}  ozzit.xlsx\n{digest}  provenance.json\n".encode()),
            {"ozzit.xlsx": digest, "provenance.json": digest},
        )


class BundleVerificationTests(ReleaseFixture):
    def setUp(self):
        super().setUp()
        self.bundle = self.directory / "bundle"
        self.create(self.bundle)

    def verify(self):
        return release_bundle.verify_bundle(self.bundle, root=self.root, check_semantics=False)

    def test_bundle_must_be_a_directory_of_regular_files(self):
        with self.assertRaisesRegex(ValueError, "^bundle must be a regular non-symlink directory$"):
            release_bundle.verify_bundle(
                self.bundle / "SHA256SUMS", root=self.root, check_semantics=False
            )
        (self.bundle / "nested").mkdir()
        with self.assertRaisesRegex(
            ValueError, "^bundle inventory may contain only regular non-symlink files$"
        ):
            self.verify()

    def test_provenance_must_be_canonical_json(self):
        path = self.bundle / "provenance.json"
        indented = json.dumps(json.loads(path.read_text(encoding="utf-8")), indent=2)
        path.write_text(indented, encoding="utf-8")
        self.write_checksums(self.bundle)
        with self.assertRaisesRegex(ValueError, "^provenance.json is not canonical JSON$"):
            self.verify()

    def test_each_provenance_claim_is_checked(self):
        pristine = (self.bundle / "provenance.json").read_bytes()
        for change, message in PROVENANCE_DEFECTS:
            with self.subTest(message=message):
                (self.bundle / "provenance.json").write_bytes(pristine)
                self.rewrite_provenance(self.bundle, change)
                with self.assertRaisesRegex(ValueError, f"^{message}"):
                    self.verify()


class CreateBundleTests(ReleaseFixture):
    def test_output_parent_must_already_exist(self):
        output = self.directory / "missing" / "bundle"
        with self.assertRaisesRegex(
            ValueError, "^release bundle parent must be an existing non-symlink directory$"
        ):
            self.create(output)
        self.assertFalse(output.parent.exists())

    def test_destination_created_during_staging_is_not_replaced(self):
        output = self.directory / "bundle"
        verify = release_bundle.verify_bundle

        def verify_then_race(bundle, **kwargs):
            provenance = verify(bundle, **kwargs)
            output.mkdir()
            (output / "keep.txt").write_text("another writer", encoding="utf-8")
            return provenance

        with mock.patch.object(release_bundle, "verify_bundle", verify_then_race):
            with self.assertRaisesRegex(FileExistsError, "destination already exists"):
                self.create(output)

        self.assertEqual([path.name for path in output.iterdir()], ["keep.txt"])
        self.assertEqual(
            [path.name for path in self.directory.iterdir() if path.name.startswith(".bundle-")],
            [],
        )


class CommandLineTests(ReleaseFixture):
    def setUp(self):
        super().setUp()
        self.output = self.directory / "out" / "bundle"
        self.output.parent.mkdir()

    def main(self, *argv):
        printed = io.StringIO()
        with (
            mock.patch.object(release_bundle, "__file__", str(self.root / "tools" / "x.py")),
            self.git(),
            self.gates(),
            contextlib.redirect_stdout(printed),
        ):
            code = release_bundle.main(list(argv))
        return code, printed.getvalue()

    def test_parser_requires_a_known_command(self):
        for argv in ([], ["publish"], ["create", "--version", "3.2.0"], ["verify"]):
            with self.subTest(argv=argv), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as caught:
                    release_bundle.build_parser().parse_args(argv)
                self.assertEqual(caught.exception.code, 2)

    def test_create_stages_from_the_clean_head(self):
        for source in ([], ["--source-commit", HEAD]):
            with self.subTest(source=source):
                shutil.rmtree(self.output, ignore_errors=True)
                code, printed = self.main(
                    "create", "--version", "3.2.0", "--output", str(self.output), *source
                )
                self.assertEqual(code, 0)
                self.assertEqual(
                    printed, f"OK: created Ozzit 3.2.0 bundle at {self.output}\n"
                )
                self.assertEqual(
                    json.loads((self.output / "provenance.json").read_bytes())["source"]["commit"],
                    HEAD,
                )

    def test_create_refuses_a_source_commit_other_than_head(self):
        with self.assertRaisesRegex(ValueError, "^source commit must equal the candidate checkout"):
            self.main(
                "create", "--version", "3.2.0", "--output", str(self.output),
                "--source-commit", "c" * 40,
            )
        self.assertFalse(self.output.exists())

    def test_create_refuses_a_dirty_checkout(self):
        self.git_answers["status"] = " M ozzit.xlsx"
        with self.assertRaisesRegex(ValueError, "must be clean before staging"):
            self.main("create", "--version", "3.2.0", "--output", str(self.output))
        self.assertEqual(self.git_calls, [["status", "--porcelain", "--untracked-files=all"]])

    def test_verify_accepts_a_bundle_matching_head_and_the_requested_claims(self):
        self.create(self.output)
        for extra in ([], ["--version", "3.2.0"], ["--source-commit", HEAD]):
            with self.subTest(extra=extra):
                code, printed = self.main("verify", "--bundle", str(self.output), *extra)
                self.assertEqual(code, 0)
                self.assertEqual(printed, f"OK: verified Ozzit 3.2.0 bundle at {self.output}\n")

    def test_verify_rejects_mismatched_claims(self):
        self.create(self.output)
        cases = (
            ({"rev-parse": "c" * 40}, [], "does not match the candidate checkout HEAD"),
            ({}, ["--version", "3.2.1"], "^bundle version does not match --version$"),
            ({}, ["--version", "v3.2.0"], "version must be canonical X.Y.Z"),
            ({}, ["--source-commit", "c" * 40], "does not match --source-commit$"),
        )
        answers = self.git_answers
        for override, extra, message in cases:
            with self.subTest(extra=extra, override=override):
                self.git_answers = {**answers, **override}
                with self.assertRaisesRegex(ValueError, message):
                    self.main("verify", "--bundle", str(self.output), *extra)

    def test_verify_reruns_the_semantic_gates(self):
        self.create(self.output)
        verify = release_bundle.verify_bundle
        with mock.patch.object(release_bundle, "verify_bundle", wraps=verify) as spy:
            self.main("verify", "--bundle", str(self.output))
        spy.assert_called_once_with(self.output, root=self.root, check_semantics=True)


class GitInvocationTests(unittest.TestCase):
    def test_git_output_runs_a_checked_utf8_query_and_trims_it(self):
        completed = SimpleNamespace(returncode=0, stdout="  answer\n")
        root = Path("checkout")
        with mock.patch.object(release_bundle.subprocess, "run", return_value=completed) as run:
            self.assertEqual(release_bundle._git_output(root, "rev-parse", "HEAD"), "answer")
        run.assert_called_once_with(
            ["git", "rev-parse", "HEAD"],
            cwd=root,
            capture_output=True,
            text=True,
            encoding="utf-8",
            check=True,
            timeout=30,
        )

    def test_failed_git_query_propagates(self):
        error = release_bundle.subprocess.CalledProcessError(128, ["git", "status"])
        with mock.patch.object(release_bundle.subprocess, "run", side_effect=error):
            with self.assertRaises(release_bundle.subprocess.CalledProcessError):
                release_bundle._git_output(Path("checkout"), "status")


if __name__ == "__main__":
    unittest.main()
