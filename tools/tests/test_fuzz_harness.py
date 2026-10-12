"""Replay the fuzz seed corpus so a broken harness fails ordinary CI."""

from __future__ import annotations

import sys
import unittest
from decimal import Decimal
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from fuzz import ozzit_parsers  # noqa: E402


class FuzzHarnessTests(unittest.TestCase):
    def test_seed_corpus(self) -> None:
        seeds = sorted((ROOT / "fuzz" / "corpus").iterdir())
        self.assertGreater(len(seeds), 0)
        for seed in seeds:
            with self.subTest(seed=seed.name):
                ozzit_parsers.test_one_input(seed.read_bytes())

    def test_a_number_without_tolerance_is_caught(self) -> None:
        with mock.patch.object(ozzit_parsers, "number", return_value=(Decimal(1), Decimal(0))):
            with self.assertRaisesRegex(AssertionError, "returned"):
                ozzit_parsers.test_one_input(b"1")

    def test_a_non_string_statement_is_caught(self) -> None:
        with mock.patch.object(ozzit_parsers, "statements", return_value=[1]):
            with self.assertRaisesRegex(AssertionError, "non-string"):
                ozzit_parsers.test_one_input(b"")


if __name__ == "__main__":
    unittest.main()
