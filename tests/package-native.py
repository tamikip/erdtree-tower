import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

script = Path(__file__).resolve().parents[1] / 'distribution/native_build.py'
spec = importlib.util.spec_from_file_location('native_build', script)
native_build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native_build)


class NativePackageTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.workspace = Path(self.temporary.name)
        self.project = self.workspace / 'outputs/erdtree-tower'
        self.vendor = self.project / 'native/vendor/hudhook'
        self.vendor.mkdir(parents=True)
        self.built = self.workspace / 'work/erdtree-native-build/release/erdtree_tower_preview.dll'
        self.built.parent.mkdir(parents=True)
        self.destination = self.workspace / 'package/erdtree_tower_party_test.dll'
        self.destination.parent.mkdir()
        self.destination.write_bytes(b'old staged DLL')
        self.checksum = self.destination.parent / 'native.sha256'
        self.metadata = {'packages': [{'name': 'erdtree-tower-preview', 'dependencies': [
            {'name': 'hudhook', 'path': str(self.vendor), 'source': None, 'features': ['dx12']}
        ]}]}

    def command(self, arguments, **kwargs):
        if arguments[1] == 'metadata':
            return subprocess.CompletedProcess(arguments, 0, json.dumps(self.metadata))
        self.built.write_bytes(b'fresh DLL: DX12 queue captured from submitted back-buffer PRESENT transition')
        return subprocess.CompletedProcess(arguments, 0)

    def test_packaging_rebuilds_and_replaces_stale_dll_and_checksum(self):
        self.built.write_bytes(b'stale previous build')
        with patch.object(native_build.subprocess, 'run', side_effect=self.command) as calls:
            digest = native_build.stage_native(self.project, self.destination, self.checksum)
        self.assertEqual(calls.call_args_list[1].args[0][1], 'build')
        self.assertEqual(self.destination.read_bytes(), self.built.read_bytes())
        self.assertEqual(digest, hashlib.sha256(self.built.read_bytes()).hexdigest())
        self.assertEqual(self.checksum.read_text().strip(), digest)

    def test_failed_build_cannot_package_previous_binary(self):
        self.built.write_bytes(b'DX12 queue captured from submitted back-buffer PRESENT transition')
        def failed(arguments, **kwargs):
            if arguments[1] == 'metadata':
                return self.command(arguments, **kwargs)
            raise subprocess.CalledProcessError(1, arguments)
        with patch.object(native_build.subprocess, 'run', side_effect=failed):
            with self.assertRaises(subprocess.CalledProcessError):
                native_build.stage_native(self.project, self.destination, self.checksum)
        self.assertEqual(self.destination.read_bytes(), b'old staged DLL')
        self.assertFalse(self.checksum.exists())

    def test_unpatched_dependency_and_missing_fix_are_rejected(self):
        self.metadata['packages'][0]['dependencies'][0]['source'] = 'git+https://github.com/veeenu/hudhook'
        with patch.object(native_build.subprocess, 'run', side_effect=self.command) as calls:
            with self.assertRaises(RuntimeError):
                native_build.build_native(self.project)
            self.assertEqual(calls.call_count, 1)
        self.metadata['packages'][0]['dependencies'][0]['source'] = None
        def unpatched(arguments, **kwargs):
            result = self.command(arguments, **kwargs)
            if arguments[1] == 'build':
                self.built.write_bytes(b'old DLL without queue fix')
            return result
        with patch.object(native_build.subprocess, 'run', side_effect=unpatched):
            with self.assertRaises(RuntimeError):
                native_build.stage_native(self.project, self.destination, self.checksum)
        self.assertEqual(self.destination.read_bytes(), b'old staged DLL')


if __name__ == '__main__':
    unittest.main()
