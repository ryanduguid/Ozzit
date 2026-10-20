"""Malformed workbook and interrupted postbuild regression checks."""

import base64
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
import verify_workbook  # noqa: E402


class ReviewRegressionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.book = self.root / 'fixture.xlsx'
        shutil.copy2(TOOLS.parent / 'ozzit.xlsx', self.book)
        with zipfile.ZipFile(self.book) as archive:
            self.parts = {name: archive.read(name) for name in archive.namelist()}

    def verify(self):
        with zipfile.ZipFile(self.book, "w") as archive:
            for name in sorted(self.parts):
                archive.writestr(name, self.parts[name])
        return subprocess.run([sys.executable, str(TOOLS / 'verify_workbook.py'), str(self.book)],
                              capture_output=True, text=True, encoding='utf-8')

    def test_malformed_xml_is_not_returned_for_later_parsing(self):
        with patch.object(verify_workbook, 'failures', []):
            self.assertIsNone(verify_workbook.check_xml_part('fixture.xml', b'<broken>'))
            self.assertEqual(len(verify_workbook.failures), 1)

    def test_malformed_afe_payloads_report_without_tracebacks(self):
        for payload in (b'x' * 151, b'x' * 150, b'{' * 150):
            with self.subTest(payload_length=len(payload)):
                self.parts['customXml/item1.xml'] = b'<root>' + base64.b64encode(payload) + b'</root>'
                result = self.verify()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('invalid AFE project store', result.stdout)
                self.assertNotIn('Traceback', result.stderr)

    def test_only_the_hidden_formula_environment_add_in_passes(self):
        self.assertEqual(self.verify().returncode, 0)
        extension = self.parts['xl/webextensions/webextension1.xml']
        panes = self.parts['xl/webextensions/taskpanes.xml']
        cases = {
            'a second add-in': ({'xl/webextensions/webextension2.xml': extension}, 'expected one web extension'),
            'a different add-in': ({'xl/webextensions/webextension1.xml': extension.replace(
                b'wa200003696', b'wa104379999')}, 'an add-in other than'),
            'a visible task pane': ({'xl/webextensions/taskpanes.xml': panes.replace(
                b'visibility="0"', b'visibility="1"')}, 'one hidden task pane'),
            'custom functions': ({'xl/webextensions/webextension1.xml': extension.replace(
                b'<we:bindings/>', b'<we:bindings/><we:containsCustomFunctions/>')},
                'containsCustomFunctions'),
            'a custom-functions property': ({'xl/webextensions/webextension1.xml': extension.replace(
                b'<we:properties>', b'<we:properties><we:property name="containsCustomFunctions" value="true"/>')},
                'containsCustomFunctions'),
            # The AFE part stays as a decoy while the pane targets a differently named add-in.
            'a decoy beside a redirected pane': ({
                'xl/webextensions/other.xml': extension.replace(b'wa200003696', b'wa104379999'),
                'xl/webextensions/_rels/taskpanes.xml.rels': self.parts[
                    'xl/webextensions/_rels/taskpanes.xml.rels'].replace(b'webextension1.xml', b'other.xml'),
            }, 'expected one web extension'),
            'a pane resolving elsewhere': ({'xl/webextensions/_rels/taskpanes.xml.rels': self.parts[
                'xl/webextensions/_rels/taskpanes.xml.rels'].replace(b'webextension1.xml', b'missing.xml')},
                'does not resolve'),
        }
        original = dict(self.parts)
        # The words alone, in a cell's text, are not a declaration.
        strings = self.parts['xl/sharedStrings.xml']
        self.parts = {**original, 'xl/sharedStrings.xml': strings.replace(
            b'</sst>', b'<si><t>containsCustomFunctions</t></si></sst>')}
        self.assertNotIn('containsCustomFunctions', self.verify().stdout)
        for label, (changes, message) in cases.items():
            with self.subTest(label):
                self.parts = {**original, **changes}
                result = self.verify()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(message, result.stdout)

    def test_each_sheet_scoped_name_is_checked(self):
        book = self.parts['xl/workbook.xml'].decode()
        names = '<definedName name="fixture" localSheetId="0">(</definedName>'
        names += '<definedName name="fixture" localSheetId="1">1</definedName>'
        self.parts['xl/workbook.xml'] = book.replace('</definedNames>', names + '</definedNames>').encode()
        result = self.verify()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('unbalanced parentheses in fixture', result.stdout)
