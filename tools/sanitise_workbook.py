"""Shared text and archive helpers for remaining Python compiler and gate callers.

The workbook sanitise pass is native: run ozzit sanitise_workbook workbook.xlsx.
"""

from __future__ import annotations

import io
import os
import stat
import tempfile
import zipfile
from pathlib import Path

FIXED_DATE = (2026, 1, 1, 0, 0, 0)


def column_number(letters: str) -> int:
    """A cell reference's column letters as a 1-based number: A is 1, AA is 27."""
    n = 0
    for ch in letters:
        n = n * 26 + ord(ch) - 64
    return n


def read_text(path: Path) -> str:
    """Read UTF-8 text with the stored line endings left exactly as they are."""
    # Build tool; the path is the repository's own file or the maintainer's argument.
    with open(path, encoding="utf-8", newline="") as handle:  # NOSONAR
        return handle.read()


def write_text(path: Path, text: str) -> None:
    """Write UTF-8 text atomically, line endings left exactly as they are."""
    replace_atomically(path, text.encode("utf-8"))


def deterministic_bytes(parts: dict[str, bytes]) -> bytes:
    """Return one canonical zip representation of a workbook's parts."""
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for name in sorted(parts):
            zi = zipfile.ZipInfo(name, date_time=FIXED_DATE)
            zi.compress_type = zipfile.ZIP_DEFLATED
            zi.create_system = 3
            zi.external_attr = 0o644 << 16
            z.writestr(zi, parts[name], compresslevel=9)
    return buffer.getvalue()


def stage_bytes(destination: Path, data: bytes) -> Path:
    """Return a closed, unique sibling containing data; the caller owns its cleanup."""
    handle = tempfile.NamedTemporaryFile(
        mode="wb", prefix=".ozzit-", suffix=".tmp",
        dir=destination.parent, delete=False,
    )
    temporary = Path(handle.name)
    try:
        with handle:
            handle.write(data)
        if os.name != "nt":
            try:
                mode = destination.stat().st_mode
            except FileNotFoundError:
                pass
            else:
                temporary.chmod(stat.S_IMODE(mode))
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise
    return temporary


def replace_atomically(workbook: Path, data: bytes) -> None:
    """Replace workbook atomically, retaining the original if replacement fails."""
    tmp = stage_bytes(workbook, data)
    try:
        # Build tool; the path is the repository's own file or the maintainer's argument.
        os.replace(tmp, workbook)  # NOSONAR
    finally:
        tmp.unlink(missing_ok=True)


def write_deterministic(workbook: Path, parts: dict[str, bytes]) -> None:
    """Atomically replace workbook with its canonical archive."""
    replace_atomically(workbook, deterministic_bytes(parts))
