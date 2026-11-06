import hashlib
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'distribution'))
import bundle_seamless


class BundleSeamless(unittest.TestCase):
    def test_stages_only_runtime_and_clean_template(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / 'source'
            for name, data in {'ersc.dll': b'verified', 'locale/english.json': b'{}',
                               'crashpad/crashpad_handler.exe': b'handler',
                               'ersc_settings.ini': b'personal secret',
                               'crashdumps/report.dmp': b'private'}.items():
                file = source / name
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_bytes(data)
            template = root / 'default.ini'
            template.write_bytes(b'cooppassword =\n')
            with patch.object(bundle_seamless, 'SHA256', hashlib.sha256(b'verified').hexdigest()):
                bundle_seamless.stage_component(source, root / 'stage', template)
            paths = {p.relative_to(root / 'stage').as_posix() for p in (root / 'stage').rglob('*') if p.is_file()}
            self.assertEqual(paths, {'ersc.dll', 'locale/english.json', 'crashpad/crashpad_handler.exe', 'default.ini'})
            (source / 'ersc.dll').write_bytes(b'wrong build')
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                bundle_seamless.stage_component(source, root / 'rejected', template)
            self.assertFalse((root / 'rejected').exists())


if __name__ == '__main__':
    unittest.main()
