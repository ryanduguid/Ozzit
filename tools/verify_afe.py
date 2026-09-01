"""Check that the Advanced Formula Environment store matches what ships.

Usage: python tools/verify_afe.py [workbook] [src dir]

The AFE task pane is the documented editing surface, but the existing gates only
compare src/ with defined names and never compare the AFE store. Every module is
held, Debt included: its schedules were rewritten with SCAN so that nothing in the
library recurses by name, which is what kept Debt out of the store before.
"""

import base64
import html
import json
import re
import sys
import zipfile
from pathlib import Path
from typing import TextIO, TypedDict, cast

WORKBOOK = sys.argv[1] if len(sys.argv) > 1 else "ozzit.xlsx"
SRC = Path(sys.argv[2] if len(sys.argv) > 2 else "src")
SCHEMA = "http://schemas.advancedformulaenvironment.officeapps.live.com/afeprojects/0.2"
MODULES = ("Dates", "Essentials", "Financial", "Ratios", "Utilities", "Debt")
AFE_BLOB_RE = rb"[A-Za-z0-9+/]{100,}={0,2}"


class AfeFile(TypedDict):
    path: str
    text: str


class AfeStore(TypedDict):
    schema: str
    files: list[AfeFile]
    projectNames: list[str]


def find_afe_blob(raw: bytes) -> bytes:
    """Return the longest base64 payload in the AFE store XML."""
    matches = re.findall(AFE_BLOB_RE, raw)
    if not matches:
        raise ValueError("AFE store has no base64 payload")
    return max(matches, key=len)


def fail(failures: list[str], message: str) -> None:
    failures.append(message)


def decode_store(workbook: Path) -> AfeStore:
    try:
        with zipfile.ZipFile(workbook) as archive:
            raw = archive.read("customXml/item1.xml")
    except (FileNotFoundError, KeyError, zipfile.BadZipFile) as exc:
        raise ValueError(f"cannot read AFE store: {exc}") from exc
    try:
        encoded = find_afe_blob(raw)
    except ValueError as exc:
        raise ValueError(str(exc)) from exc
    try:
        store: object = json.loads(base64.b64decode(encoded).decode("utf-16-le"))
    except (ValueError, UnicodeDecodeError) as exc:
        raise ValueError(f"cannot decode AFE store: {exc}") from exc
    if not isinstance(store, dict):
        raise ValueError("cannot decode AFE store: root must be an object")
    if not isinstance(store.get("schema"), str):
        raise ValueError("cannot decode AFE store: schema must be a string")
    files = store.get("files")
    if not isinstance(files, list):
        raise ValueError("cannot decode AFE store: files must be a list")
    for index, item in enumerate(files):
        if not isinstance(item, dict):
            raise ValueError(f"cannot decode AFE store: files[{index}] must be an object")
        if not isinstance(item.get("path"), str) or not isinstance(item.get("text"), str):
            raise ValueError(
                f"cannot decode AFE store: files[{index}] path and text must be strings"
            )
    project_names = store.get("projectNames")
    if not isinstance(project_names, list) or not all(
        isinstance(name, str) for name in project_names
    ):
        raise ValueError("cannot decode AFE store: projectNames must be a list of strings")
    return cast(AfeStore, store)


def workbook_names(workbook: Path) -> set[str]:
    with zipfile.ZipFile(workbook) as archive:
        text = archive.read("xl/workbook.xml").decode("utf-8")
    names = set()
    for raw in re.findall(r'<definedName\b[^>]*\bname="([^"]+)"[^>]*>', text):
        name = html.unescape(raw)
        if name.startswith("oz."):
            names.add(name)
    return names


def check(workbook: Path, src: Path) -> list[str]:
    failures: list[str] = []
    try:
        store = decode_store(workbook)
    except ValueError as exc:
        return [str(exc)]

    if store["schema"] != SCHEMA:
        fail(failures, f"AFE schema is {store['schema']!r}")

    files = {item["path"]: item["text"] for item in store["files"]}
    for module in MODULES:
        path = f"/projects/{module}"
        expected_path = src / f"{module}.txt"
        if path not in files:
            fail(failures, f"AFE module {path} missing")
            continue
        try:
            expected = expected_path.read_text(encoding="utf-8")
        except FileNotFoundError:
            fail(failures, f"source module {expected_path} missing")
            continue
        if files[path] != expected:
            first = next(
                (
                    index
                    for index, pair in enumerate(zip(files[path], expected, strict=False))
                    if pair[0] != pair[1]
                ),
                min(len(files[path]), len(expected)),
            )
            actual = files[path][max(0, first - 30) : first + 50].replace("\n", " ")
            wanted = expected[max(0, first - 30) : first + 50].replace("\n", " ")
            fail(
                failures,
                f"AFE module {path} differs from src/{module}.txt at character {first}: "
                f"{actual!r} != {wanted!r}",
            )

    expected_names = workbook_names(workbook)
    actual_names = set(store["projectNames"])
    for name in sorted(expected_names - actual_names):
        fail(failures, f"AFE projectNames is missing shipped name {name}")
    for name in sorted(actual_names - expected_names):
        fail(failures, f"AFE projectNames has unknown name {name}")

    return failures


def emit_line(text: str, stream: TextIO | None = None) -> None:
    """Write one line without losing unencodable diagnostic characters."""
    stream = sys.stdout if stream is None else stream
    encoding = getattr(stream, "encoding", None) or "utf-8"
    try:
        text.encode(encoding)
    except UnicodeEncodeError:
        text = text.encode(encoding, errors="backslashreplace").decode(encoding)
    print(text, file=stream)


def main() -> None:
    failures = check(Path(WORKBOOK), SRC)
    if failures:
        emit_line(f"FAIL: {len(failures)} AFE problem(s) in {WORKBOOK}")
        for failure in failures:
            emit_line(f"  - {failure}")
        sys.exit(1)
    emit_line(f"OK: AFE store in {WORKBOOK} matches all {len(MODULES)} modules and every shipped name")


if __name__ == "__main__":
    main()
