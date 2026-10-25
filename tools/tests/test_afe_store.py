import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class AfeGateTests(unittest.TestCase):
    def test_tracked_debt_module_no_longer_recurses_by_name(self):
        text = (ROOT / "src" / "Debt.txt").read_text(encoding="utf-8")
        for name in ("AmortiseBλ", "DebtSculptFixedλ", "DebtSculptVariableλ",
                     "DebtSculptVariableLRVλ", "InterestLRVλ"):
            body = text.split(f"\n{name} = LAMBDA(", 1)[1].split("\n);", 1)[0]
            # help text names the function; only code outside string literals can call it
            code = re.sub(r'"(?:[^"]|"")*"', '""', body)
            self.assertNotIn(f"{name}(", code, f"{name} still calls itself")


if __name__ == "__main__":
    unittest.main()
