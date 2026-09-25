"""Check the retained v3.4.2 native evidence without running Excel or pyxirr."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
from pathlib import Path
from typing import Any

import pyxirr_comparison as comparison

ROOT = Path(__file__).resolve().parents[1]
RELEASE_SHA256 = "0306793a7e473ce70e78149fea1e107fc0f714d61ab50960c16f6fd528878f6f"
COMPARISON = "docs/pyxirr-release-v3.4.2.json"
INSTALLATION = "docs/install-v3.4.2.json"
RUNNERS = ("tools/pyxirr_comparison.py", "tools/excel_eval_formulas.ps1")
INSTALL_RUNNER = "tools/excel_install_selftest.ps1"
INSTALL_NAMES = {"rolling-sum": ["oz.RollingSumλ"],
                 "amortise-helpers": ["oz.Amortiseλ", "oz.TimelineOffsetλ", "oz.TimelinePositionλ"]}
INSTALL_INPUTS = {
    "rolling-sum": ("oz.RollingSumλ!H21", "=oz.RollingSumλ({1,2,3,4},2)",
                    "=oz.RollingSumλ({2,4,6,8},2)", 1, 4),
    "amortise-helpers": ("oz.Amortiseλ!I24", "=oz.Amortiseλ(1000,0.06,4,DATE(2026,7,1))",
                        "=oz.Amortiseλ(2000,0.06,4,DATE(2026,7,1))", 6, 4),
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_record(path: Path) -> dict[str, Any]:
    def unique(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            require(key not in result, "duplicate evidence key")
            result[key] = value
        return result

    data = json.loads(path.read_text(encoding="utf-8-sig"), object_pairs_hook=unique)
    require(isinstance(data, dict), "evidence must be an object")
    return data


def finite(value: Any) -> bool:
    return type(value) in (int, float) and math.isfinite(value)


def check_grid(grid: dict[str, Any]) -> None:
    require(all(type(grid[key]) is int and grid[key] > 0 for key in ("rows", "columns")),
            "invalid installation spill shape")
    values = grid["values"]
    require(isinstance(values, list) and len(values) == grid["rows"] * grid["columns"]
            and all(finite(value) for value in values), "invalid installation spill values")


def verify(root: Path = ROOT, *, workbook: Path | None = None,
           destination: Path | None = None) -> list[str]:
    trial = read_record(root / COMPARISON)
    install = read_record(root / INSTALLATION)
    require(trial["workbook_sha256"] == trial["expected_sha256"] == install["source_sha256"]
            == RELEASE_SHA256, "evidence does not identify the retained v3.4.2 workbook")
    require(trial["pyxirr"] == comparison.PYXIRR, "pyxirr version changed")
    require(isinstance(trial["runner_commit"], str)
            and re.fullmatch(r"[0-9a-f]{40}", trial["runner_commit"]) is not None,
            "invalid runner commit")
    require(all(isinstance(item["excel"], str) and item["excel"].strip()
                for item in (trial, install)), "missing Excel build")
    require(set(trial["runner_files"]) == set(RUNNERS), "comparison runner inventory changed")
    for name in RUNNERS:
        require(sha256(root / name) == trial["runner_files"][name], f"runner evidence is stale: {name}")
    require(sha256(root / INSTALL_RUNNER) == install["runner_sha256"],
            f"runner evidence is stale: {INSTALL_RUNNER}")

    cases = trial["cases"]
    require(isinstance(cases, list) and [(case["id"], case["formula"]) for case in cases]
            == [(case["id"], case["formula"]) for case in comparison.CASES],
            "comparison cases are missing, duplicated or changed")
    for case, defined in zip(cases, comparison.CASES):
        expected_values = {"amortise-monthly": 300, "amortise-quarterly-deferred": 60}.get(defined["id"], 1)
        require(type(case["values"]) is int and case["values"] == expected_values,
                "invalid comparison value count")
        require(type(case["agrees"]) is bool, "comparison agreement must be boolean")
        tolerance = comparison.RATE_TOLERANCE if defined["kind"] == "rate" else comparison.MONEY_TOLERANCE
        if case["agrees"]:
            require(finite(case["max_abs_diff"]) and 0 <= case["max_abs_diff"] <= tolerance
                    and case["tolerance"] == tolerance and case["note"] is None,
                    "comparison difference exceeds its declared tolerance")
            if case["values"] == 1:
                require(finite(case["ozzit"]) and finite(case["pyxirr"]), "non-numeric comparison result")
                require(abs(abs(case["ozzit"] - case["pyxirr"]) - case["max_abs_diff"]) <= 1e-15,
                        "comparison difference contradicts its results")
        else:
            require(defined["id"] == "irr-two-roots" and case["ozzit"] == "#NUM!"
                    and finite(case["pyxirr"]) and finite(case["pyxirr_npv"])
                    and abs(case["pyxirr_npv"]) <= comparison.MONEY_TOLERANCE
                    and case["max_abs_diff"] is None
                    and case["note"] == defined["expected_difference"],
                    "comparison has an unsupported difference")

    require(install["passed"] is True and install["source_unchanged"] is True
            and type(install["external_links"]) is int and install["external_links"] == 0,
            "installation did not pass without source changes or external links")
    require(isinstance(install["destination_sha256"], str)
            and re.fullmatch(r"[0-9a-f]{64}", install["destination_sha256"]) is not None,
            "invalid installed workbook digest")
    require(isinstance(install["cases"], list)
            and [case["id"] for case in install["cases"]] == list(INSTALL_NAMES),
            "installation case inventory changed")
    for case in install["cases"]:
        require(case["passed"] is True and case["required_names"] == INSTALL_NAMES[case["id"]],
                "installation case failed or required names changed")
        source, formula, changed_formula, rows, columns = INSTALL_INPUTS[case["id"]]
        require((case["copied_from"], case["formula"], case["changed_formula"])
                == (source, formula, changed_formula), "installation inputs changed")
        before, after = case["reopened"], case["changed_input"]
        check_grid(before)
        check_grid(after)
        require((before["rows"], before["columns"]) == (after["rows"], after["columns"])
                == (rows, columns), "installation spill shape changed")
        require(any(value != 0 for value in before["values"])
                and all(abs(changed - 2 * original) <= comparison.MONEY_TOLERANCE
                        for original, changed in zip(before["values"], after["values"])),
                "installation changed-input values contradict the doubled inputs")

    messages = ["Retained v3.4.2 evidence: 8 comparison cases and 2 installation cases checked; runners match."]
    current = sha256(root / "ozzit.xlsx")
    messages.append("The retained workbook matches this checkout." if current == RELEASE_SHA256 else
                    "Historical release evidence: the checkout workbook differs; these records do not validate it.")
    for label, path, digest in (("release workbook", workbook, RELEASE_SHA256),
                                ("installed workbook", destination, install["destination_sha256"])):
        if path is not None:
            require(sha256(path) == digest, f"{label} hash does not match the retained evidence")
            messages.append(f"Supplied {label} bytes match the retained evidence.")
    messages.append("No native execution performed. Unprovided release or installed files are not verified.")
    return messages


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workbook", type=Path, help="optional exact v3.4.2 release workbook")
    parser.add_argument("--destination", type=Path, help="optional workbook from the recorded installation")
    args = parser.parse_args(argv)
    try:
        for message in verify(workbook=args.workbook, destination=args.destination):
            print(message)
    except (OSError, ValueError, KeyError, TypeError, OverflowError) as error:
        print(f"FAIL: native evidence: {error}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
