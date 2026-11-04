"""Record a bounded execution comparison from the published QuantLib wheel."""

import argparse
import hashlib
import json
import platform
import sys
import tempfile
import zipfile
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("wheel", type=Path, help="The pinned Linux QuantLib 1.43 wheel")
parser.add_argument("output", type=Path, help="A new file for the reference execution record")
args = parser.parse_args()
WHEEL_SHA256 = "35cd2c178aa9a30c3b7a9ca050536037c3185afd6728e6a1cf2085adef54464e"
if hashlib.sha256(args.wheel.read_bytes()).hexdigest() != WHEEL_SHA256:
    parser.error("wheel hash differs from the recorded QuantLib artefact")
if args.output.exists():
    parser.error("output already exists; retain it and choose a new path")
workspace = tempfile.TemporaryDirectory(prefix="quantlib-comparison-")
ROOT = Path(workspace.name)
with zipfile.ZipFile(args.wheel) as archive:
    archive.extractall(ROOT / "quantlib-wheel")
sys.path.insert(0, str(ROOT / "quantlib-wheel"))
import QuantLib as ql  # noqa: E402

intervals = [
    ("zero_leap_day", "2024-02-29", "2024-02-29"),
    ("ordinary_month", "2026-07-01", "2026-08-01"),
    ("february_common", "2023-02-01", "2023-03-01"),
    ("february_leap", "2024-02-01", "2024-03-01"),
    ("full_leap_year", "2024-01-01", "2025-01-01"),
    ("month_end_31", "2026-01-31", "2026-03-31"),
    ("common_february_end", "2023-02-28", "2023-03-31"),
    ("leap_february_end", "2024-02-29", "2024-03-31"),
    ("leap_day_to_august_end", "2024-02-29", "2024-08-31"),
    ("one_day_year_boundary", "2023-12-31", "2024-01-01"),
    ("split_common_leap", "2023-12-01", "2024-02-01"),
    ("multiple_calendar_years", "2023-07-01", "2026-07-01"),
    ("leap_century", "2000-02-28", "2000-03-01"),
    ("common_century", "2100-02-28", "2100-03-01"),
    ("across_leap_century", "1999-12-01", "2001-01-01"),
    ("across_common_century", "2099-12-01", "2100-03-01"),
    ("first_supported_year", "1901-01-01", "1902-01-01"),
    ("long_interval", "1901-01-01", "2199-12-31"),
]
counters = {
    "1": ql.Thirty360(ql.Thirty360.European),
    "2": ql.Actual360(False),
    "3": ql.Actual365Fixed(ql.Actual365Fixed.Standard),
    "4": ql.ActualActual(ql.ActualActual.ISDA),
}


def day(text):
    year, month, day_of_month = map(int, text.split("-"))
    return ql.Date(day_of_month, month, year)


provenance = {
    "version": "1.43",
    "filename": args.wheel.name,
    "url": "https://files.pythonhosted.org/packages/2e/39/2df89a3f4fe6668535d7c56f29ee646f7642e46b7fc6a9db1707804c7520/quantlib-1.43-cp39-abi3-manylinux_2_24_x86_64.manylinux_2_28_x86_64.whl",
    "sha256": WHEEL_SHA256,
}
binary = ROOT / "quantlib-wheel" / "QuantLib" / "_QuantLib.abi3.so"
cases = [
    {
        "id": name,
        "start": start,
        "following": following,
        "fractions": {
            method: counter.yearFraction(day(start), day(following))
            for method, counter in counters.items()
        },
    }
    for name, start, following in intervals
]
controls = [
    {
        "case_id": "common_february_end",
        "method": "1",
        "alternative": "Thirty360(USA)",
        "fraction": ql.Thirty360(ql.Thirty360.USA).yearFraction(
            day("2023-02-28"), day("2023-03-31")
        ),
    },
    {
        "case_id": "leap_february_end",
        "method": "1",
        "alternative": "Thirty360(ISDA)",
        "fraction": ql.Thirty360(ql.Thirty360.ISDA).yearFraction(
            day("2024-02-29"), day("2024-03-31")
        ),
    },
    {
        "case_id": "split_common_leap",
        "method": "4",
        "alternative": "ActualActual(ISMA)",
        "fraction": ql.ActualActual(ql.ActualActual.ISMA).yearFraction(
            day("2023-12-01"), day("2024-02-01")
        ),
    },
]
controls.extend([
    {"case_id": "common_february_end", "method": "1", "alternative": "Thirty360(BondBasis)",
     "fraction": ql.Thirty360(ql.Thirty360.BondBasis).yearFraction(day("2023-02-28"), day("2023-03-31"))},
    {"case_id": "zero_leap_day", "method": "2", "alternative": "Actual360(True)",
     "fraction": ql.Actual360(True).yearFraction(day("2024-02-29"), day("2024-02-29"))},
    {"case_id": "full_leap_year", "method": "3", "alternative": "Actual365Fixed(NoLeap)",
     "fraction": ql.Actual365Fixed(ql.Actual365Fixed.NoLeap).yearFraction(day("2024-01-01"), day("2025-01-01"))},
    {"case_id": "full_leap_year", "method": "3", "alternative": "ActualActual(ISDA)",
     "fraction": ql.ActualActual(ql.ActualActual.ISDA).yearFraction(day("2024-01-01"), day("2025-01-01"))},
    {"case_id": "split_common_leap", "method": "4", "alternative": "ActualActual(AFB)",
     "fraction": ql.ActualActual(ql.ActualActual.AFB).yearFraction(day("2023-12-01"), day("2024-02-01"))},
])
result = {
    "schema_version": 1,
    "quantlib_version": ql.__version__,
    "wheel": provenance,
    "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
    "generator_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    "python_version": platform.python_version(),
    "platform": platform.platform(),
    "interval_semantics": "[start, following)",
    "constructor_expressions": {"1": "Thirty360(European)", "2": "Actual360(False)",
                                "3": "Actual365Fixed(Standard)", "4": "ActualActual(ISDA)"},
    "conventions": {method: counter.name() for method, counter in counters.items()},
    "cases": cases,
    "variant_controls": controls,
}
assert ql.__version__ == provenance["version"]
assert all(
    abs(next(c for c in cases if c["id"] == control["case_id"])["fractions"][control["method"]]
        - control["fraction"]) > 1e-8
    for control in controls
)
target = args.output
target.write_text(json.dumps(result, indent=2) + "\n")
workspace.cleanup()
print(f"QUANTLIB_EXECUTION_RECORDED: {len(cases)} intervals, {len(cases) * len(counters)} values")
