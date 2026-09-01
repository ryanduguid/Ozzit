"""Synchronise the Advanced Formula Environment store with src/.

Usage: python tools/sync_afe_store.py [workbook] [src dir]

All six modules are rewritten, Debt included now that nothing in it recurses by
name. The pass preserves the store's schema, locale, project name order and
JSON layout, re-encodes UTF-16LE/base64, and writes through the canonical
archive writer.

The store holds two views of the same library: the module texts, and a flat
projectNames index. verify_afe.py gates both, so both are synchronised here.
Only the texts were, until a function was added and the index did not follow.
A name still shipping keeps its place in the index, so the existing grouping
survives; a new one is filed after the last name from its own module, and a
name that no longer ships is dropped.
"""

from __future__ import annotations

import base64
import json
import sys
import zipfile
from pathlib import Path

from sanitise_workbook import write_deterministic
from verify_afe import MODULES, find_afe_blob, workbook_names
from verify_sources import NAME, statements

NAMESPACE = "oz"


def module_of(src: Path) -> dict[str, str]:
    """Every shipped name mapped to the module that declares it."""
    out = {}
    for path in sorted(src.glob("*.txt")):
        for statement in statements(path.read_text(encoding="utf-8")):
            match = NAME.match(statement)
            if match:
                out[f"{NAMESPACE}.{match.group(1)}"] = path.stem
    return out


def index_order(current: list[str], expected: set[str], owner: dict[str, str]) -> list[str]:
    """The projectNames index, keeping what is there and filing what is not."""
    kept = [name for name in current if name in expected]
    for name in sorted(expected - set(kept), key=str.lower):
        module = owner.get(name)
        after = [i for i, held in enumerate(kept) if owner.get(held) == module]
        kept.insert(after[-1] + 1 if after else len(kept), name)
    return kept


def sync(workbook: Path, src: Path) -> list[str]:
    with zipfile.ZipFile(workbook) as archive:
        parts = {name: archive.read(name) for name in archive.namelist()}
    raw = parts["customXml/item1.xml"]
    encoded = find_afe_blob(raw)
    store = json.loads(base64.b64decode(encoded).decode("utf-16-le"))

    files = {item.get("path"): item for item in store.get("files", [])}
    changes = []
    for module in MODULES:
        expected = (src / f"{module}.txt").read_text(encoding="utf-8")
        path = f"/projects/{module}"
        if path not in files:
            item = {"path": path, "text": expected}
            store["files"].append(item)
            files[path] = item
            changes.append(f"added AFE module {path}")
        elif files[path].get("text") != expected:
            files[path]["text"] = expected
            changes.append(f"synced AFE module {path}")

    # The flat index the task pane reads, which the gate checks against the workbook.
    shipped = workbook_names(workbook)
    current = list(store.get("projectNames", []))
    wanted = index_order(current, shipped, module_of(src))
    if wanted != current:
        added = len(set(wanted) - set(current))
        dropped = len(set(current) - set(wanted))
        store["projectNames"] = wanted
        changes.append(
            f"synced AFE projectNames: {added} added, {dropped} dropped, "
            f"{len(wanted)} listed"
        )

    replacement = base64.b64encode(
        json.dumps(store, ensure_ascii=False, separators=(",", ":")).encode("utf-16-le")
    )
    if replacement == encoded and not changes:
        return ["AFE store already matches src/"]
    parts["customXml/item1.xml"] = raw.replace(encoded, replacement)
    write_deterministic(workbook, parts)
    return changes


def main() -> None:
    if len(sys.argv) > 3:
        sys.exit("FAIL: usage: python tools/sync_afe_store.py [workbook] [src dir]")
    workbook = Path(sys.argv[1] if len(sys.argv) > 1 else "ozzit.xlsx")
    src = Path(sys.argv[2] if len(sys.argv) > 2 else "src")
    if not workbook.is_file():
        sys.exit(f"FAIL: no such workbook: {workbook}")
    try:
        changes = sync(workbook, src)
    except (
        OSError,
        FileNotFoundError,
        KeyError,
        ValueError,
        json.JSONDecodeError,
        zipfile.BadZipFile,
    ) as exc:
        sys.exit(f"FAIL: cannot sync AFE store: {exc}")
    for change in changes:
        print(change)
    print(f"OK: {workbook} AFE store synced, {workbook.stat().st_size:,} bytes")


if __name__ == "__main__":
    main()
