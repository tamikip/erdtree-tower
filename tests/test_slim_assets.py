import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('slim_assets', Path(__file__).resolve().parents[1] / 'distribution/slim_assets.py')
slim = importlib.util.module_from_spec(spec)
spec.loader.exec_module(slim)


class SlimAssets(unittest.TestCase):
    def exercise(self, failure, english=False):
        with tempfile.TemporaryDirectory(prefix='tower-slim-test-') as directory:
            root = Path(directory)
            project, stage, game = root / 'project', root / 'stage', root / 'game'
            (project / 'licenses').mkdir(parents=True)
            (project / 'licenses/bcdec.txt').write_text('license')
            icon_bytes = b'\x12\x34\x56\xff' * 25600
            icon_root = stage / 'scene-assets/reward-icons'
            icon_root.mkdir(parents=True)
            for icon in (123, 456, 990000):
                (icon_root / f'{icon}.rgba').write_bytes(icon_bytes)
            targets=(*slim.TARGETS,slim.ENGLISH_THEME) if english else slim.TARGETS
            low_files = ('menu/low/01_common.tpf.dcx', 'menu/low/01_common.sblytbnd.dcx', 'locales/en-US/menu/low/01_common.tpf.dcx')
            for relative in (*targets, *low_files):
                path = stage / 'scene-assets' / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(relative.encode())

            def run(command, **kwargs):
                if 'run' in command:
                    folder = stage / 'asset-patches'
                    folder.mkdir()
                    manifest = {'Files': [{'Path': p, 'TargetHash': hashlib.sha256(p.encode()).hexdigest()} for p in targets]}
                    manifest['Icons'] = [{'Id': 123, 'Sha256': hashlib.sha256(icon_bytes).hexdigest()}]
                    (folder / 'manifest.json').write_text(json.dumps(manifest))
                elif 'build' in command:
                    folder = Path(command[-1]); folder.mkdir(exist_ok=True)
                    for name in ('Tower.AssetRestore.exe', 'Tower.AssetRestore.exe.config'):
                        (folder / name).write_bytes(b'helper')
                else:
                    if failure == 'runtime':
                        raise subprocess.CalledProcessError(1, command)
                    for relative in targets:
                        path = Path(command[1]) / 'scene-assets' / relative
                        path.parent.mkdir(parents=True, exist_ok=True)
                        path.write_bytes(b'wrong' if failure == 'hash' else relative.encode())
                    path = Path(command[1]) / 'scene-assets/reward-icons/123.rgba'
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(b'wrong' if failure == 'icon' else icon_bytes)

            def codec(project, folder):
                folder.mkdir(parents=True)
                path = folder / 'Tower.IconCodec.dll'
                path.write_bytes(b'codec')
                return path

            with patch.object(slim.subprocess, 'run', side_effect=run), patch.object(slim, 'build_icon_codec', side_effect=codec):
                if failure:
                    with self.assertRaises((ValueError, subprocess.CalledProcessError)):
                        slim.prepare(project, stage, game)
                    for relative in targets:
                        self.assertEqual((stage / 'scene-assets' / relative).read_bytes(), relative.encode())
                    self.assertFalse((stage / 'asset-mode.txt').exists())
                    for relative in low_files:
                        self.assertTrue((stage / 'scene-assets' / relative).exists())
                    self.assertEqual((icon_root / '123.rgba').read_bytes(), icon_bytes)
                else:
                    slim.prepare(project, stage, game)
                    for relative in targets:
                        self.assertFalse((stage / 'scene-assets' / relative).exists())
                    self.assertEqual((stage / 'asset-mode.txt').read_text().strip(), 'local-v1')
                    self.assertTrue((stage / 'Tower.AssetRestore.exe').exists())
                    for relative in low_files:
                        self.assertFalse((stage / 'scene-assets' / relative).exists())
                    self.assertFalse(any('/low/' in f['Path'] for f in json.loads((stage / 'asset-patches/manifest.json').read_text())['Files']))
                    self.assertFalse((icon_root / '123.rgba').exists())
                    self.assertEqual((icon_root / '456.rgba').read_bytes(), icon_bytes)
                    self.assertEqual((icon_root / '990000.rgba').read_bytes(), icon_bytes)

    def test_removes_only_verified_full_assets(self): self.exercise(None)
    def test_generation_failure_preserves_full_assets(self): self.exercise('runtime')
    def test_wrong_output_hash_preserves_full_assets(self): self.exercise('hash')
    def test_wrong_icon_hash_preserves_full_assets(self): self.exercise('icon')
    def test_english_theme_is_reconstructed_instead_of_shipping_base_assets(self): self.exercise(None,english=True)
    def test_english_reconstruction_failure_preserves_assets(self): self.exercise('hash',english=True)
