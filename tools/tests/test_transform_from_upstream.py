"""Regression fixtures for the pure paths in transform_from_upstream.py.

The builder is an intentionally one-shot migration over an upstream workbook
that is not redistributed. Importing it would execute that migration. These
tests compile only named function definitions and their literal dependencies
from the tracked source, so the assertions exercise the exact implementation
without inventing or redistributing the upstream input.
"""

from __future__ import annotations

import ast
from collections.abc import Mapping
import datetime
from pathlib import Path
import re
from types import SimpleNamespace
from typing import Any
import unittest


ROOT = Path(__file__).resolve().parents[2]
TRANSFORM = ROOT / "tools" / "transform_from_upstream.py"


def load_transform_symbols(*function_names: str) -> SimpleNamespace:
    """Load exact tracked helper definitions without running the migration."""

    dependencies = {
        "REPO_URL",
        "TODAY_AU",
        "BRAND",
        "BRAND_BARE",
        "TYPOS",
        "AMORT",
        "AMERICAN",
        "AU_SUB",
        "AU_WORD",
        "AU_DATA",
        "URL_RE",
        "YEAR_SHIFT",
        "EPOCH",
        "DEPREFIX",
        "CACHED_VALUE",
    }
    wanted_functions = set(function_names)
    source = TRANSFORM.read_text(encoding="utf-8")
    tree = ast.parse(source, filename=str(TRANSFORM))
    selected: list[ast.stmt] = []

    for node in tree.body:
        if isinstance(node, ast.FunctionDef) and node.name in wanted_functions:
            selected.append(node)
            continue
        if isinstance(node, (ast.Assign, ast.AnnAssign)):
            targets = node.targets if isinstance(node, ast.Assign) else [node.target]
            names = {
                target.id
                for target in targets
                if isinstance(target, ast.Name)
            }
            if names & dependencies:
                selected.append(node)

    loaded = {
        node.name
        for node in selected
        if isinstance(node, ast.FunctionDef)
    }
    missing = wanted_functions - loaded
    if missing:
        raise AssertionError(f"transform helpers missing from source: {sorted(missing)}")

    namespace: dict[str, Any] = {
        "Any": Any,
        "Mapping": Mapping,
        "datetime": datetime,
        "re": re,
    }
    module = ast.fix_missing_locations(ast.Module(body=selected, type_ignores=[]))
    exec(compile(module, str(TRANSFORM), "exec"), namespace)
    return SimpleNamespace(**namespace)


class DateTransformTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.helpers = load_transform_symbols(
            "_shift_ymd",
            "_shift_serial",
            "_mdy_to_au",
            "_iso_shift",
            "_arr_shift",
            "transform_text",
        )

    def test_shift_ymd_moves_an_ordinary_date_two_years(self) -> None:
        self.assertEqual(self.helpers._shift_ymd(2023, 8, 18), (2025, 8, 18))

    def test_shift_ymd_clamps_only_a_leap_day(self) -> None:
        self.assertEqual(self.helpers._shift_ymd(2024, 2, 29), (2026, 2, 28))

    def test_shift_ymd_rejects_an_invalid_month(self) -> None:
        with self.assertRaises(ValueError):
            self.helpers._shift_ymd(2024, 13, 1)

    def test_shift_ymd_rejects_an_invalid_day_that_is_not_leap_day(self) -> None:
        with self.assertRaises(ValueError):
            self.helpers._shift_ymd(2024, 4, 31)

    def test_shift_serial_preserves_the_time_fraction(self) -> None:
        epoch = datetime.datetime(1899, 12, 30)
        original = datetime.datetime(2024, 1, 15, 12, 0)
        serial = (original - epoch).days + 0.5
        shifted = self.helpers._shift_serial(serial)
        expected = (datetime.datetime(2026, 1, 15) - epoch).days + 0.5
        self.assertEqual(shifted, expected)

    def test_shift_serial_clamps_a_leap_day_and_keeps_fraction(self) -> None:
        epoch = datetime.datetime(1899, 12, 30)
        original = datetime.datetime(2024, 2, 29)
        serial = (original - epoch).days + 0.25
        shifted = self.helpers._shift_serial(serial)
        expected = (datetime.datetime(2026, 2, 28) - epoch).days + 0.25
        self.assertEqual(shifted, expected)

    def test_mdy_callback_converts_to_australian_day_first_order(self) -> None:
        match = re.search(r"(\d+)/(\d+)/(\d+)", "2/26/2023")
        self.assertIsNotNone(match)
        self.assertEqual(self.helpers._mdy_to_au(match), "26/2/2025")

    def test_mdy_callback_expands_a_two_digit_year(self) -> None:
        match = re.search(r"(\d+)/(\d+)/(\d+)", "2/26/23")
        self.assertIsNotNone(match)
        self.assertEqual(self.helpers._mdy_to_au(match), "26/2/2025")

    def test_mdy_callback_rejects_a_malformed_calendar_date(self) -> None:
        match = re.search(r"(\d+)/(\d+)/(\d+)", "13/1/2024")
        self.assertIsNotNone(match)
        with self.assertRaises(ValueError):
            self.helpers._mdy_to_au(match)

    def test_iso_callback_preserves_iso_shape(self) -> None:
        match = re.search(r"(\d{4})-(\d{2})-(\d{2})", "2023-02-26")
        self.assertIsNotNone(match)
        self.assertEqual(self.helpers._iso_shift(match), "2025-02-26")

    def test_array_callback_shifts_only_plausible_excel_serials(self) -> None:
        match = re.search(r"\{[\d;\s]+\}", "{ 45000; 12; 49000 }")
        self.assertIsNotNone(match)
        result = self.helpers._arr_shift(match)
        self.assertNotIn("45000", result)
        self.assertIn("12", result)
        self.assertIn("49000", result)

    def test_transform_text_is_deterministic_for_the_same_fixture(self) -> None:
        fixture = (
            '<t lang="en-US">BXD. gray modeling 2/26/2023 2023-02-26 '
            'https://gist.github.com/example/abc</t>'
        )
        first = self.helpers.transform_text(fixture)
        second = self.helpers.transform_text(fixture)
        self.assertEqual(first, second)

    def test_transform_text_rebrands_and_localises_prose(self) -> None:
        fixture = (
            '<t lang="en-US">BXD. gray modeling</t>'
            '<v>Paycheck</v><v>Gas</v>'
        )
        result = self.helpers.transform_text(fixture)
        self.assertIn('lang="en-AU"', result)
        self.assertIn("ozzit.d.", result)
        self.assertIn("grey", result)
        self.assertIn("modelling", result)
        self.assertIn("Pay", result)
        self.assertIn("Petrol", result)
        self.assertNotIn("Paycheck", result)

    def test_transform_text_replaces_repository_urls(self) -> None:
        fixture = (
            "https://sites.google.com/site/beyondexcel/functions "
            "https://gist.github.com/owner/deadbeef"
        )
        result = self.helpers.transform_text(fixture)
        self.assertEqual(
            result,
            "https://github.com/ryanduguid/Ozzit "
            "https://github.com/ryanduguid/Ozzit",
        )

    def test_transform_text_shifts_quoted_sample_dates(self) -> None:
        fixture = '<t>"2/26/2023"</t><t>"2023-02-26"</t>'
        result = self.helpers.transform_text(fixture)
        self.assertIn('"26/2/2025"', result)
        self.assertIn('"2025-02-26"', result)

    def test_transform_text_updates_version_lines_to_the_fixed_build_date(self) -> None:
        fixture = "VERSION: →Aug 18 2024"
        self.assertEqual(
            self.helpers.transform_text(fixture),
            "VERSION: →18 Aug 2026",
        )

    def test_transform_text_removes_a_duplicate_about_website_row(self) -> None:
        repository = "https://github.com/ryanduguid/Ozzit"
        fixture = f'"Website: →{repository} ¶" &amp; "next"'
        result = self.helpers.transform_text(fixture)
        self.assertEqual(result, '"next"')

    def test_transform_text_does_not_touch_unquoted_slash_dates(self) -> None:
        fixture = "period 2/26/2023 remains explanatory prose"
        self.assertEqual(self.helpers.transform_text(fixture), fixture)


class XmlAndHelpTransformTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.helpers = load_transform_symbols(
            "xesc",
            "help_lines",
            "build_xml",
            "build_afe",
        )

    @staticmethod
    def spec() -> dict[str, Any]:
        return {
            "module": "ozzit.f",
            "name": "Exampleλ",
            "sig": "Exampleλ(Value)",
            "desc": "Returns Value & keeps XML safe.",
            "params": [("Value", "(Required) A value < 10.")],
            "example": "ozzit.f.Exampleλ(5)",
            "result": "5",
            "xml_decl": "_xlop.Value",
            "help_test": "_xlfn.ISOMITTED(_xlpm.Value)",
            "lets": [("Return input", "Result", "_xlpm.Value")],
        }

    def test_xesc_escapes_all_three_xml_metacharacters(self) -> None:
        self.assertEqual(
            self.helpers.xesc('A&B<C>D'),
            "A&amp;B&lt;C&gt;D",
        )

    def test_xesc_leaves_quotes_for_excel_formula_literals(self) -> None:
        self.assertEqual(self.helpers.xesc('"quoted"'), '"quoted"')

    def test_help_lines_have_stable_sections_and_parameter_rows(self) -> None:
        lines = self.helpers.help_lines(self.spec())
        self.assertEqual(lines[0], "FUNCTION:      →Exampleλ(Value)¶")
        self.assertEqual(lines[1], "DESCRIPTION:   →Returns Value & keeps XML safe.¶")
        self.assertEqual(lines[4], "PARAMETERS:    →¶")
        self.assertIn("Value", lines[5])
        self.assertEqual(lines[-1], "→5")

    def test_build_xml_escapes_help_and_keeps_stored_prefixes(self) -> None:
        xml = self.helpers.build_xml(self.spec())
        self.assertIn("_xlfn.LAMBDA(_xlop.Value", xml)
        self.assertIn("&amp;", xml)
        self.assertIn("&lt;", xml)
        self.assertNotIn("A value < 10", xml)

    def test_build_xml_is_deterministic(self) -> None:
        self.assertEqual(
            self.helpers.build_xml(self.spec()),
            self.helpers.build_xml(self.spec()),
        )

    def test_build_afe_removes_stored_prefixes(self) -> None:
        source = self.helpers.build_afe(self.spec())
        self.assertIn("Exampleλ = LAMBDA", source)
        self.assertIn("ISOMITTED(Value)", source)
        self.assertNotIn("_xlfn.", source)
        self.assertNotIn("_xlpm.", source)
        self.assertNotIn("_xlop.", source)

    def test_build_afe_emits_each_parameter_once(self) -> None:
        source = self.helpers.build_afe(self.spec())
        declaration = source[source.index("LAMBDA(") : source.index("LET(")]
        self.assertEqual(declaration.count("[Value]"), 1)

    def test_build_afe_omits_revision_history(self) -> None:
        source = self.helpers.build_afe(self.spec())
        self.assertNotIn("REVISIONS", source)
        self.assertNotIn("REVISION", source)


class CachedValueTransformTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.helpers = load_transform_symbols(
            "cached_forms",
            "refresh_cached",
            "left_of",
            "_split_args",
        )

    def test_cached_forms_normalises_a_whole_help_label(self) -> None:
        self.assertEqual(
            self.helpers.cached_forms('"Life     →'),
            (True, "Life"),
        )

    def test_cached_forms_keeps_a_substring_fragment(self) -> None:
        self.assertEqual(
            self.helpers.cached_forms("old phrase"),
            (False, "old phrase"),
        )

    def test_refresh_cached_changes_values_without_changing_formulas(self) -> None:
        fixture = (
            '<c r="A1"><f>old phrase</f><v>old phrase</v></c>'
            '<c r="A2"><v>prefix old phrase suffix</v></c>'
        )
        result, hits = self.helpers.refresh_cached(
            fixture,
            False,
            "old phrase",
            "new phrase",
        )
        self.assertEqual(hits, 2)
        self.assertIn("<f>old phrase</f>", result)
        self.assertEqual(result.count("new phrase"), 2)

    def test_refresh_cached_whole_mode_ignores_partial_values(self) -> None:
        fixture = "<v>Label</v><v>Label extra</v><v> Label </v>"
        result, hits = self.helpers.refresh_cached(
            fixture,
            True,
            "Label",
            "Renamed",
        )
        self.assertEqual(hits, 2)
        self.assertIn("<v>Label extra</v>", result)
        self.assertIn("<v>Renamed</v>", result)
        self.assertIn("<v> Renamed </v>", result)

    def test_refresh_cached_is_idempotent_after_success(self) -> None:
        fixture = "<v>old</v><v>old</v>"
        first, first_hits = self.helpers.refresh_cached(
            fixture,
            False,
            "old",
            "new",
        )
        second, second_hits = self.helpers.refresh_cached(
            first,
            False,
            "old",
            "new",
        )
        self.assertEqual(first_hits, 2)
        self.assertEqual(second_hits, 0)
        self.assertEqual(second, first)

    def test_refresh_cached_reports_zero_for_an_absent_value(self) -> None:
        fixture = "<v>untouched</v>"
        result, hits = self.helpers.refresh_cached(
            fixture,
            False,
            "missing",
            "replacement",
        )
        self.assertEqual(result, fixture)
        self.assertEqual(hits, 0)

    def test_left_of_handles_single_and_multiple_letter_columns(self) -> None:
        cases = {
            "B1": "A1",
            "Z9": "Y9",
            "AA10": "Z10",
            "BA27": "AZ27",
            "AAA3": "ZZ3",
        }
        for supplied, expected in cases.items():
            with self.subTest(supplied=supplied):
                self.assertEqual(self.helpers.left_of(supplied), expected)

    def test_left_of_rejects_the_first_column(self) -> None:
        with self.assertRaises(AssertionError):
            self.helpers.left_of("A1")

    def test_left_of_rejects_malformed_references(self) -> None:
        for supplied in ("", "1A", "A", "A-1", "A 1", "a1"):
            with self.subTest(supplied=supplied), self.assertRaises(ValueError):
                self.helpers.left_of(supplied)

    def test_left_of_rejects_row_zero(self) -> None:
        with self.assertRaises(ValueError):
            self.helpers.left_of("B0")

    def test_split_args_handles_nested_calls(self) -> None:
        formula = "CALL(1, IF(A1, SUM(2,3), 4), 5) trailing"
        start = formula.index("(")
        args, end = self.helpers._split_args(formula, start)
        self.assertEqual(args, ["1", " IF(A1, SUM(2,3), 4)", " 5"])
        self.assertEqual(formula[end:], " trailing")

    def test_split_args_keeps_commas_inside_strings(self) -> None:
        formula = 'CALL("a,b", "c""d", 3)'
        args, end = self.helpers._split_args(formula, formula.index("("))
        self.assertEqual(args, ['"a,b"', ' "c""d"', " 3"])
        self.assertEqual(end, len(formula))

    def test_split_args_handles_an_empty_final_argument(self) -> None:
        formula = "CALL(1,2,)"
        args, _ = self.helpers._split_args(formula, formula.index("("))
        self.assertEqual(args, ["1", "2", ""])

    def test_split_args_rejects_an_unbalanced_call(self) -> None:
        formula = "CALL(1, IF(2,3)"
        with self.assertRaisesRegex(ValueError, "unbalanced call"):
            self.helpers._split_args(formula, formula.index("("))


class TransformSourceContractTests(unittest.TestCase):
    def test_source_keeps_the_transform_helpers_typed(self) -> None:
        tree = ast.parse(TRANSFORM.read_text(encoding="utf-8"))
        functions = [
            node
            for node in ast.walk(tree)
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
        ]
        self.assertEqual(len(functions), 36)
        for function in functions:
            with self.subTest(function=function.name, line=function.lineno):
                parameters = [
                    *function.args.posonlyargs,
                    *function.args.args,
                    *function.args.kwonlyargs,
                ]
                if function.args.vararg:
                    parameters.append(function.args.vararg)
                if function.args.kwarg:
                    parameters.append(function.args.kwarg)
                self.assertIsNotNone(function.returns)
                self.assertTrue(all(arg.annotation is not None for arg in parameters))

    def test_source_does_not_execute_dynamic_version_or_shell_commands(self) -> None:
        source = TRANSFORM.read_text(encoding="utf-8")
        self.assertNotIn("eval(", source)
        self.assertNotIn("exec(", source)
        self.assertNotIn("shell=True", source)
        self.assertNotIn("subprocess", source)


if __name__ == "__main__":
    unittest.main()
