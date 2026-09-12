"""Malformed workbook and interrupted postbuild regression checks."""

import base64
import re
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
import sanitise_workbook
import verify_workbook
sys.path.insert(0, str(TOOLS / 'postbuild'))
import aasb16_leases
import remove_residue
import strip_revision_history
import workbook_palette


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
        sanitise_workbook.write_deterministic(self.book, self.parts)
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

    def test_each_sheet_scoped_name_is_checked(self):
        book = self.parts['xl/workbook.xml'].decode()
        names = '<definedName name="fixture" localSheetId="0">(</definedName>'
        names += '<definedName name="fixture" localSheetId="1">1</definedName>'
        self.parts['xl/workbook.xml'] = book.replace('</definedNames>', names + '</definedNames>').encode()
        result = self.verify()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('unbalanced parentheses in fixture', result.stdout)

    def test_other_binary_parts_keep_the_content_type_default(self):
        content_type = b'<Types><Default Extension="bin" ContentType="application/octet-stream"/></Types>'
        parts = {'xl/customProperty1.bin': b'fabricated',
                 'xl/printerSettings/printerSettings1.bin': b'fabricated',
                 '[Content_Types].xml': content_type}
        remove_residue.drop_custom_properties(parts, [])
        self.assertEqual(parts['[Content_Types].xml'], content_type)
        self.assertNotIn('xl/customProperty1.bin', parts)

    def test_palette_does_not_claim_an_indexed_background_was_folded(self):
        styles = self.parts['xl/styles.xml'].decode()
        styles = styles.replace('</fills>', '<fill><patternFill><bgColor indexed="42"/></patternFill></fill></fills>')
        self.parts['xl/styles.xml'] = styles.encode()
        sanitise_workbook.write_deterministic(self.book, self.parts)
        before = self.book.read_bytes()
        self.assertEqual(workbook_palette.run(self.book), [])
        self.assertEqual(self.book.read_bytes(), before)

    def test_revision_removal_preserves_crlf_outside_the_removed_block(self):
        source = '/* Keep this.\r\n REVISIONS:\r\n Old history.\r\n*/\r\nFixture = 1;\r\n'
        actual, count = strip_revision_history.strip_module(source)
        self.assertEqual(count, 1)
        self.assertEqual(actual, '/* Keep this.\r\n*/\r\nFixture = 1;\r\n')

    def test_lease_pass_restores_source_if_workbook_write_fails(self):
        source = self.root / (aasb16_leases.MODULE + '.txt')
        source.write_bytes(b'Fixture = 1;\r\n')
        before = source.read_bytes()
        with (
            patch.object(aasb16_leases, 'src_state', return_value='absent'),
            patch.object(aasb16_leases, 'workbook_state', return_value='absent'),
            patch.object(aasb16_leases, 'build_definitions', return_value={}),
            patch.object(aasb16_leases, 'add_to_src', return_value='changed'),
            patch.object(aasb16_leases, 'add_to_workbook', return_value='<workbook/>'),
            patch.object(aasb16_leases, 'write_deterministic', side_effect=OSError('synthetic failure')),
        ):
            with self.assertRaisesRegex(OSError, 'synthetic failure'):
                aasb16_leases.run(self.book, self.root, None)
        self.assertEqual(source.read_bytes(), before)

    def test_window_geometry_is_replaced_after_unrelated_attributes(self):
        book = self.parts['xl/workbook.xml'].decode()
        book = re.sub(r'<workbookView\b[^>]*/>',
                      '<workbookView visibility="visible" xWindow="99" yWindow="88" windowWidth="1" windowHeight="2"/>', book)
        self.parts['xl/workbook.xml'] = book.encode()
        sanitise_workbook.write_deterministic(self.book, self.parts)
        sanitise_workbook.sanitise(self.book)
        with zipfile.ZipFile(self.book) as archive:
            book = archive.read('xl/workbook.xml').decode()
        view = re.search(r'<workbookView\b[^>]*/>', book).group()
        self.assertEqual(view.count('xWindow='), 1)
        self.assertIn('visibility="visible"', view)
        self.assertIn(sanitise_workbook.FIXED_WINDOW, view)
