"""Fuzz the module statement splitter and the help-result number parser."""

from __future__ import annotations

import sys
from decimal import Decimal
from pathlib import Path

# The fuzzing job installs only Atheris; the tools import each other from tools/.
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))

from generate_selftest_examples import number  # noqa: E402
from verify_sources import statements  # noqa: E402


def test_one_input(data: bytes) -> None:
    text = data.decode("utf-8", errors="replace")
    # Explicit raises, because python -O strips assert statements.
    if not all(isinstance(statement, str) for statement in statements(text)):
        raise AssertionError("statements() returned a non-string")
    for line in text.splitlines():
        # A printed result either reads as a finite number with a tolerance or
        # is not a number; it never raises.
        parsed = number(line)
        if parsed is None:
            continue
        value, tolerance = parsed
        if not (value.is_finite() and tolerance > Decimal(0)):
            raise AssertionError(f"number({line!r}) returned {parsed!r}")


if __name__ == "__main__":
    import atheris

    atheris.instrument_all()
    atheris.Setup(sys.argv, test_one_input)
    atheris.Fuzz()
