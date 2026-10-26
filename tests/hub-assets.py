from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
HUB = 'm11_10_00_00.msb.dcx'


class HubAssetsTests(unittest.TestCase):
    def test_hub_map_is_shipped_with_the_source_assets(self):
        self.assertTrue((ROOT / 'scene-assets/map/mapstudio' / HUB).is_file(),
                        'The map-native hub NPCs require the hub map patch')

    @unittest.skipUnless(os.name == 'nt', 'Windows launcher integration')
    def test_launcher_rejects_a_package_missing_only_the_hub_map(self):
        game = Path('F:/SteamLibrary/steamapps/common/ELDEN RING/Game')
        if not (game / 'eldenring.exe').is_file():
            self.skipTest('Local game installation unavailable')
        release = ROOT / 'release-preview'
        if not (release / 'loader').is_dir():
            self.skipTest('Built runtime package unavailable')
        scratch = ROOT / 'target'
        scratch.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=scratch) as directory:
            package = Path(directory)
            shutil.copy2(ROOT / 'Launch-Preview.ps1', package / 'Launch-Preview.ps1')
            shutil.copy2(ROOT / 'preview.me3', package / 'preview.me3')
            for folder in ('scene-assets', 'loader'):
                shutil.copytree(release / folder, package / folder, copy_function=os.link)
            for file in release.glob('erdtree_tower_preview_v*.dll'):
                os.link(file, package / file.name)
            hub = package / 'scene-assets/map/mapstudio' / HUB
            if hub.exists():
                hub.unlink()
            command = ['powershell.exe', '-NoProfile', '-ExecutionPolicy', 'Bypass',
                       '-File', str(package / 'Launch-Preview.ps1'), '-GameDir', str(game), '-CheckOnly']
            missing = subprocess.run(command, capture_output=True)
            self.assertNotEqual(missing.returncode, 0, 'Launcher accepted a package without the hub NPC map')
            self.assertIn(b'Launch-Preview.ps1', missing.stderr)
            shutil.copy2(ROOT / 'scene-assets/map/mapstudio' / HUB, hub)
            complete = subprocess.run(command, capture_output=True)
            self.assertEqual(complete.returncode, 0, complete.stderr.decode(errors='replace'))


if __name__ == '__main__':
    unittest.main()
