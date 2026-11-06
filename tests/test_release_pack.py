import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location('release_pack', Path(__file__).resolve().parents[1] / 'distribution/release_pack.py')
pack = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pack)


def write(root, files):
    for rel, data in files.items():
        path = Path(root) / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)


class ReleasePack(unittest.TestCase):
    def test_private_seamless_config_and_runtime_are_rejected(self):
        for relative in ('config/SeamlessCoop/ersc_settings.ini', 'seamless-entry/runtime/SeamlessCoop/ersc_settings.ini'):
            with self.subTest(relative=relative), tempfile.TemporaryDirectory() as tmp:
                write(tmp, {relative: b'private password'})
                with self.assertRaisesRegex(ValueError, 'Private file'):
                    pack.collect(Path(tmp))
    def test_display_version_does_not_replace_build_number(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(root / 'stage', {'mod.dll': b'mod'})
            manifest = pack.build(root / 'stage', 97, root / 'out', display_version='1.2.3')
            self.assertEqual((manifest['version'], manifest['displayVersion'], manifest['tag']), (97, '1.2.3', 'v97'))
            with self.assertRaises(ValueError):
                pack.build(root / 'stage', 98, root / 'bad', display_version='1.02.3')
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def test_second_release_only_packs_new_content(self):
        big = bytes(range(256)) * 4000
        write(self.root / 'v1', {'a.dll': b'dll one' * 100, 'scene-assets/icons/1.rgba': big, 'scene-assets/same.txt': b'x' * 5000})
        write(self.root / 'v2', {'a.dll': b'dll two' * 100, 'scene-assets/icons/1.rgba': big, 'scene-assets/copy.txt': b'x' * 5000, 'new.txt': b'fresh'})
        first = pack.build(self.root / 'v1', 1, self.root / 'out1')
        second = pack.build(self.root / 'v2', 2, self.root / 'out2', previous=self.root / 'out1' / pack.MANIFEST)
        fresh = {sha: b for sha, b in second['blobs'].items() if b['tag'] == 'v2'}
        # 未变化的大文件与内容相同的新路径都指向第一版的数据包，第二版只放改动的 DLL 与新文件
        self.assertEqual(len(fresh), 2)
        self.assertEqual(second['blobs'][hashlib.sha256(big).hexdigest()]['tag'], 'v1')
        packs = {'v1': (self.root / 'out1' / pack.pack_name(1)).read_bytes(), 'v2': (self.root / 'out2' / pack.pack_name(2)).read_bytes()}
        for entry in second['files']:
            blob = second['blobs'][entry['sha256']]
            data = pack.read_blob(packs[blob['tag']], blob)
            self.assertEqual(hashlib.sha256(data).hexdigest(), entry['sha256'])
            self.assertEqual(data, (self.root / 'v2' / entry['path']).read_bytes())
        self.assertEqual(json.loads((self.root / 'out2' / pack.MANIFEST).read_text(encoding='utf-8'))['version'], 2)
        self.assertEqual(first['pack']['size'], len(packs['v1']))

    def test_incompressible_content_is_stored(self):
        import os
        noise = os.urandom(4096)
        write(self.root / 'v1', {'noise.bin': noise})
        manifest = pack.build(self.root / 'v1', 1, self.root / 'out')
        self.assertEqual(next(iter(manifest['blobs'].values()))['method'], 'store')

    def test_private_files_and_old_versions_are_rejected(self):
        write(self.root / 'v1', {'GamePath.txt': b'D:/game'})
        with self.assertRaises(ValueError):
            pack.build(self.root / 'v1', 1, self.root / 'out')
        write(self.root / 'v2', {'a.txt': b'a'})
        pack.build(self.root / 'v2', 5, self.root / 'out5')
        with self.assertRaises(ValueError):
            pack.build(self.root / 'v2', 5, self.root / 'again', previous=self.root / 'out5' / pack.MANIFEST)


if __name__ == '__main__':
    unittest.main()
