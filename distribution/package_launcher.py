"""Build a launcher-only update while preserving the complete Mod manifest."""
import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET

from release_pack import build, collect


def is_launcher(path):
    name = path.lower()
    return name in ('erdtreetowerlauncher.exe', 'erdtreetowerlauncher.exe.config',
                    'start-tower.ps1', 'launch-logged.ps1', 'tower-loader.ps1',
                    'launch-preview.ps1', 'launch-seamless-tower.ps1', 'start-tower.cmd',
                    'start-solo.cmd', 'start-seamless.cmd') or name.startswith(('launcher/', 'loader/yafsml/'))


def build_update(stage, previous, version, launcher_version, output, notes=''):
    old = json.loads(Path(previous).read_text(encoding='utf-8'))
    if old.get('schema') != 1 or not old.get('files') or not old.get('blobs'):
        raise ValueError('A complete previous release manifest is required')
    if version <= old['version']:
        raise ValueError('Release build number must increase')
    if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', launcher_version):
        raise ValueError('Invalid launcher version')
    if old.get('launcherVersion') and tuple(map(int, launcher_version.split('.'))) <= tuple(map(int, old['launcherVersion'].split('.'))):
        raise ValueError('Launcher version must increase')
    fresh = collect(Path(stage))
    if any(not is_launcher(f['path']) for f in fresh):
        raise ValueError('Launcher-only stage contains Mod or unrelated files')
    if not any(f['path'] == 'ErdtreeTowerLauncher.exe' for f in fresh):
        raise ValueError('Missing launcher executable')
    retained = [f for f in old['files'] if not is_launcher(f['path']) and f['path'] != 'SHA256SUMS.txt']
    names = [f['path'].lower() for f in old['files']]
    if len(names) != len(set(names)) or any(f['sha256'] not in old['blobs'] for f in old['files']):
        raise ValueError('Invalid previous file list')
    output = Path(output)
    if output.exists():
        raise ValueError('Output already exists; use a new release directory')
    with tempfile.TemporaryDirectory(prefix='tower-launcher-stage-') as tmp:
        staged = Path(tmp)
        shutil.copytree(stage, staged, dirs_exist_ok=True)
        entries = sorted(retained + fresh, key=lambda f: f['path'])
        sums = ''.join(f"{f['sha256']}  {f['path']}\n" for f in entries)
        (staged / 'SHA256SUMS.txt').write_text(sums, encoding='utf-8')
        result = build(staged, version, output, previous, notes, old.get('displayVersion', ''))
    result['files'] = sorted(result['files'] + retained, key=lambda f: f['path'])
    for entry in retained:
        result['blobs'][entry['sha256']] = old['blobs'][entry['sha256']]
    result['launcherVersion'] = launcher_version
    result['updateKind'] = 'launcher'
    (output / 'tower-release.json').write_text(json.dumps(result, ensure_ascii=False, indent=1), encoding='utf-8')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--previous', required=True, type=Path, help='Complete latest tower-release.json')
    parser.add_argument('--build-number', required=True, type=int)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--notes', type=Path)
    args = parser.parse_args()
    if args.output.exists():
        parser.error('Output already exists')
    project = Path(__file__).resolve().parents[1]
    csproj = project / 'launcher/Tower.Launcher/Tower.Launcher.csproj'
    version = ET.parse(csproj).findtext('.//Version')
    old = json.loads(args.previous.read_text(encoding='utf-8'))
    if not any(f['path'] == 'SeamlessCoop/ersc.dll' for f in old['files']):
        parser.error('Unified entry requires bundled Seamless: publish a full package update first')
    with tempfile.TemporaryDirectory(prefix='tower-launcher-build-') as tmp:
        root = Path(tmp)
        published = root / 'publish'
        subprocess.run(['dotnet', 'publish', str(csproj), '-c', 'Release', '-o', str(published)], check=True)
        stage = root / 'stage'
        (stage / 'launcher').mkdir(parents=True)
        for name in ('ErdtreeTowerLauncher.exe', 'ErdtreeTowerLauncher.exe.config'):
            shutil.copy2(published / name, stage / name)
        for dll in published.glob('*.dll'):
            if dll.name != 'Microsoft.Web.WebView2.Wpf.dll':
                shutil.copy2(dll, stage / 'launcher' / dll.name)
        shutil.copy2(published / 'runtimes/win-x64/native/WebView2Loader.dll', stage / 'launcher/WebView2Loader.dll')
        shutil.copytree(published / 'launcher/ui', stage / 'launcher/ui')
        shutil.copy2(published / 'launcher/sources.json', stage / 'launcher/sources.json')
        for name in ('Start-Tower.ps1', 'Launch-Logged.ps1', 'Tower-Loader.ps1'):
            shutil.copy2(project / 'distribution' / name, stage / name)
        for name in ('Launch-Preview.ps1', 'Launch-Seamless-Tower.ps1'):
            shutil.copy2(project / name, stage / name)
        (stage / 'Start-Tower.cmd').write_text(
            '@echo off\n"%SystemRoot%\\System32\\WindowsPowerShell\\v1.0\\powershell.exe" '
            '-NoProfile -ExecutionPolicy Bypass -File "%~dp0Start-Tower.ps1" %*\n', encoding='ascii')
        shutil.copytree(project / 'loader/yafsml', stage / 'loader/yafsml')
        release = build_update(stage, args.previous, args.build_number, version, args.output,
                               args.notes.read_text(encoding='utf-8') if args.notes else '')
    mirror = args.output / 'mirror/releases'
    for folder in (mirror / 'download' / release['tag'], mirror / 'latest/download'):
        folder.mkdir(parents=True)
        shutil.copy2(args.output / 'tower-release.json', folder / 'tower-release.json')
    shutil.copy2(args.output / release['pack']['asset'], mirror / 'download' / release['tag'] / release['pack']['asset'])
    print(f"Launcher {version}; Mod {release['displayVersion']}; {len(release['files'])} files preserved in full manifest")


if __name__ == '__main__':
    main()
