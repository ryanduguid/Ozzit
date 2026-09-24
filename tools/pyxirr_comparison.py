"""Compare Ozzit's cash-flow functions in desktop Excel with pyxirr, an independent library.

Run from the repository root on a Windows host with desktop Excel and no Excel open:

    uv run --no-project --with pyxirr==0.10.8 python tools/pyxirr_comparison.py

Excel evaluates each case's formula against ozzit.xlsx through excel_eval_formulas.ps1,
which opens the workbook read-only and checks its hash is unchanged. pyxirr computes the
same quantity from the same inputs. The script writes docs/pyxirr-comparison.json and the
results table in docs/pyxirr-comparison.md. CI does not run Excel or pyxirr: the unit test
checks that the recorded evidence covers exactly these cases and that the table matches it.

A second implementation is not the truth. A disagreement is recorded with its cause, not
tuned away; a case whose two answers are both valid (two roots of one cash flow) is shown as
a convention difference and checked by the net present value at each answer instead.
"""

from __future__ import annotations

import importlib
import importlib.metadata
import json
import subprocess
import sys
import tempfile
from datetime import date
from pathlib import Path
from typing import Any, Callable

ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / "docs" / "pyxirr-comparison.json"
DOCUMENT = ROOT / "docs" / "pyxirr-comparison.md"
TABLE_START = "<!-- results:start -->"
TABLE_END = "<!-- results:end -->"
PYXIRR = "0.10.8"
RATE_TOLERANCE = 1e-7
MONEY_TOLERANCE = 1e-6
EXCEL_ERRORS = {-2146826281: "#DIV/0!", -2146826246: "#N/A", -2146826259: "#NAME?",
                -2146826252: "#NUM!", -2146826273: "#VALUE!"}

Reference = Callable[[Any], list[float]]


def monthly(start: date, count: int) -> list[date]:
    return [date(start.year + (start.month - 1 + k) // 12, (start.month - 1 + k) % 12 + 1, 1)
            for k in range(count)]


def irr(values: list[float], dates: list[date]) -> Reference:
    """IRRλ drops zero values, then calls XIRR on the rest."""
    kept = [(d, v) for d, v in zip(dates, values) if v != 0]
    return lambda px: [px.xirr([d for d, _ in kept], [v for _, v in kept])]


def schedule(principal: float, apr: float, term: int, periods: int, start: int,
             months: int) -> Reference:
    """AmortiseBλ rows (opening, interest, repayment, closing, principal) for every model
    period: nil before the start period and after the term, and the level payment on the
    declining balance in between."""
    rate = apr / 12 * months

    def rows(px: Any) -> list[float]:
        out: list[float] = []
        for period in range(1, periods + 1):
            k = period - start + 1
            if not 1 <= k <= term:
                out += [0.0] * 5
                continue
            interest = float(px.ipmt(rate, k, term, principal)) * -1
            principal_part = float(px.ppmt(rate, k, term, principal)) * -1
            closing = float(px.fv(rate, k, px.pmt(rate, term, principal), principal)) * -1
            out += [closing + principal_part, interest, interest + principal_part, closing,
                    principal_part]
        return out
    return rows


MONTHS_FROM_JULY_2026 = monthly(date(2026, 7, 1), 12)
IRREGULAR = [date(2026, 7, 1), date(2026, 9, 1), date(2027, 3, 30), date(2027, 7, 15),
             date(2027, 9, 1)]
ANNUAL = [date(2026, 7, 1), date(2027, 7, 1), date(2028, 7, 1)]

CASES: list[dict[str, Any]] = [
    {
        "id": "irr-help-example",
        "what": "IRRλ help example: investment starts in period 3 of a monthly timeline",
        "formula": "=oz.IRRλ({0,0,-150,0,-100,10,20,30,40,50,60,70},"
                   " EDATE(DATE(2026,7,1), SEQUENCE(1,24,0)))",
        "kind": "rate",
        "reference": irr([0, 0, -150, 0, -100, 10, 20, 30, 40, 50, 60, 70],
                         MONTHS_FROM_JULY_2026),
    },
    {
        "id": "irr-irregular-dates",
        "what": "IRRλ on irregular dates over 14 months",
        "formula": "=oz.IRRλ({-10000,2750,4250,3250,2750},"
                   " DATE({2026,2026,2027,2027,2027},{7,9,3,7,9},{1,1,30,15,1}))",
        "kind": "rate",
        "reference": irr([-10000, 2750, 4250, 3250, 2750], IRREGULAR),
    },
    {
        "id": "irr-two-roots",
        "what": "IRRλ on a non-conventional cash flow with two valid rates",
        "formula": "=oz.IRRλ({-100,230,-132}, DATE({2026,2027,2028},7,1))",
        "kind": "roots",
        "values": [-100, 230, -132],
        "dates": ANNUAL,
        "reference": irr([-100, 230, -132], ANNUAL),
        "expected_difference": "Excel's XIRR returns #NUM! for this cash flow with any guess "
                               "tried (0.05, 0.1, 0.25), and IRRλ passes that on. pyxirr "
                               "returns one of the two valid rates. Neither is wrong; a model "
                               "with sign changes like these needs a stated rule for which "
                               "rate it uses.",
    },
    {
        "id": "lease-arrears",
        "what": "LeaseLiabilityλ help example: three payments in arrears at 5% a period",
        "formula": "=oz.LeaseLiabilityλ({100,100,100}, 0.05)",
        "kind": "money",
        "reference": lambda px: [-px.pv(0.05, 3, 100)],
    },
    {
        "id": "lease-advance",
        "what": "LeaseLiabilityλ in advance: the measurement-date payment is excluded",
        "formula": "=oz.LeaseLiabilityλ({100,100,100}, 0.05, TRUE)",
        "kind": "money",
        "reference": lambda px: [-px.pv(0.05, 3, 100, pmt_at_beginning=True) - 100],
    },
    {
        "id": "lease-60-months",
        "what": "LeaseLiabilityλ over 60 monthly payments at a 6.5% effective annual rate",
        "formula": "=oz.LeaseLiabilityλ(SEQUENCE(1,60,1500,0), oz.PeriodRateλ(0.065))",
        "kind": "money",
        "reference": lambda px: [-px.pv(1.065 ** (1 / 12) - 1, 60, 1500)],
    },
    {
        "id": "amortise-monthly",
        "what": "AmortiseBλ: $250,000 at 6.2% APR over 60 monthly periods",
        "formula": "=oz.AmortiseBλ(250000, 6.2%, 60, 60)",
        "kind": "money",
        "columns": 60,
        "reference": schedule(250000, 0.062, 60, 60, 1, 1),
    },
    {
        "id": "amortise-quarterly-deferred",
        "what": "AmortiseBλ: $100,000 at 5% APR, 8 quarterly repayments from period 3 of 12",
        "formula": "=oz.AmortiseBλ(100000, 5%, 8, 12, 3, 3)",
        "kind": "money",
        "columns": 12,
        "reference": schedule(100000, 0.05, 8, 12, 3, 3),
    },
]


def excel_error(value: Any) -> str | None:
    if isinstance(value, str) and value.startswith("#ERR:"):
        code = int(value[5:])
        return EXCEL_ERRORS.get(code, f"error {code}")
    return None


def excel_values(case: dict[str, Any], grid: list[list[Any]]) -> list[float]:
    """Flatten the Excel result in the order the reference produces it."""
    if "columns" not in case:
        return [float(v) for row in grid for v in row]
    if len(grid[0]) != case["columns"]:
        raise SystemExit(f"{case['id']}: {len(grid[0])} Excel columns, "
                         f"expected {case['columns']}")
    return [float(grid[r][c]) for c in range(case["columns"]) for r in range(len(grid))]


def evaluate() -> dict[str, Any]:
    with tempfile.TemporaryDirectory() as tmp:
        cases = Path(tmp) / "cases.json"
        out = Path(tmp) / "out.json"
        cases.write_text(json.dumps([{"id": c["id"], "formula": c["formula"]} for c in CASES],
                                    ensure_ascii=False), encoding="utf-8")
        try:
            subprocess.run(["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass",
                            "-File", str(ROOT / "tools" / "excel_eval_formulas.ps1"),
                            "-Cases", str(cases), "-Out", str(out)], check=True, timeout=600)
        except subprocess.TimeoutExpired:
            # Killing PowerShell skips its finally block, so the automation Excel it started
            # can outlive it. It is the EXCEL.EXE whose command line carries /automation.
            raise SystemExit("FAIL: the Excel evaluation did not finish within 600s. Check "
                             "for a modal dialog, then close the hidden EXCEL.EXE started "
                             "with /automation before retrying.") from None
        result: dict[str, Any] = json.loads(out.read_text(encoding="utf-8"))
        return result


def compare(excel: dict[str, Any], px: Any) -> list[dict[str, Any]]:
    records = []
    for case in CASES:
        grid = excel["results"][case["id"]]
        reference = [float(v) for v in case["reference"](px)]
        record: dict[str, Any] = {"id": case["id"], "what": case["what"],
                                  "formula": case["formula"]}
        error = excel_error(grid[0][0])
        if error:
            # No number to compare. For a roots case, still prove pyxirr's answer is a root.
            npv = (abs(float(px.xnpv(reference[0], case["dates"], case["values"])))
                   if case["kind"] == "roots" else None)
            record.update(values=1, ozzit=error, pyxirr=reference[0], max_abs_diff=None,
                          pyxirr_npv=npv, measure="rate", agrees=False,
                          note=case.get("expected_difference"))
            records.append(record)
            continue
        ozzit = excel_values(case, grid)
        record["values"] = len(ozzit)
        if case["kind"] == "roots":
            # Both answers must be roots; they need not be the same root.
            npv = [abs(float(px.xnpv(rate, case["dates"], case["values"])))
                   for rate in (ozzit[0], reference[0])]
            record.update(ozzit=ozzit[0], pyxirr=reference[0], max_abs_diff=max(npv),
                          tolerance=MONEY_TOLERANCE, measure="NPV at each rate")
        else:
            if len(ozzit) != len(reference):
                raise SystemExit(f"{case['id']}: {len(ozzit)} Excel values, "
                                 f"{len(reference)} pyxirr values")
            diff = max(abs(a - b) for a, b in zip(ozzit, reference))
            tolerance = RATE_TOLERANCE if case["kind"] == "rate" else MONEY_TOLERANCE
            record.update(ozzit=ozzit[0] if len(ozzit) == 1 else None,
                          pyxirr=reference[0] if len(reference) == 1 else None,
                          max_abs_diff=diff, tolerance=tolerance,
                          measure="rate" if case["kind"] == "rate" else "amount")
        record["agrees"] = record["max_abs_diff"] <= record["tolerance"]
        record["note"] = None if record["agrees"] else case.get("expected_difference")
        records.append(record)
    return records


def show(value: Any) -> str:
    if value is None:
        return "schedule"
    return value if isinstance(value, str) else f"{value:.10g}"


def table(evidence: dict[str, Any]) -> str:
    lines = ["| Case | Compared | Values | Ozzit (Excel) | pyxirr | Largest difference | Agrees |",
             "| --- | --- | --- | --- | --- | --- | --- |"]
    for r in evidence["cases"]:
        diff = "n/a" if r["max_abs_diff"] is None else f"{r['max_abs_diff']:.3g}"
        agrees = "yes" if r["agrees"] else ("no, see note" if r["note"] else "NO")
        lines.append(f"| `{r['id']}` | {r['measure']} | {r['values']} | {show(r['ozzit'])} | "
                     f"{show(r['pyxirr'])} | {diff} | {agrees} |")
    notes = [f"- `{r['id']}`: {r['note']}" for r in evidence["cases"] if r["note"]]
    return "\n".join(lines + ([""] + notes if notes else []))


def write_table(evidence: dict[str, Any]) -> None:
    text = DOCUMENT.read_text(encoding="utf-8")
    head, rest = text.split(TABLE_START, 1)
    _, tail = rest.split(TABLE_END, 1)
    DOCUMENT.write_text(f"{head}{TABLE_START}\n{table(evidence)}\n{TABLE_END}{tail}",
                        encoding="utf-8", newline="\n")


def main() -> int:
    px = importlib.import_module("pyxirr")
    installed = importlib.metadata.version("pyxirr")
    if installed != PYXIRR:
        print(f"FAIL: pyxirr {installed} installed; this comparison pins {PYXIRR}")
        return 1
    excel = evaluate()
    commit = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True,
                            text=True, check=True).stdout.strip()
    evidence = {"recorded": date.today().isoformat(), "excel": excel["excel"],
                "workbook": "ozzit.xlsx", "commit": commit,
                "workbook_sha256": excel["workbook_sha256"], "pyxirr": PYXIRR,
                "cases": compare(excel, px)}
    EVIDENCE.write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + "\n",
                        encoding="utf-8", newline="\n")
    write_table(evidence)
    print(table(evidence))
    # A difference fails the run unless the case documents why it is expected.
    return 0 if all(r["agrees"] or r["note"] for r in evidence["cases"]) else 1


if __name__ == "__main__":
    sys.exit(main())
