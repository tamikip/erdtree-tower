"""Replace PC runtime archives with verified patches and omit unused Low menu overrides."""
from pathlib import Path
import hashlib
import json
import shutil
import subprocess
import uuid
import os

TARGETS = (
    'menu/hi/01_common.tpf.dcx',
    'sfx/sfxbnd_commoneffects.ffxbnd.dcx',
)
ENGLISH_THEME = 'locales/en-US/menu/hi/01_common.tpf.dcx'


def build_icon_codec(project, output):
    output.mkdir(parents=True, exist_ok=True)
    vswhere = Path(os.environ.get('ProgramFiles(x86)', r'C:\Program Files (x86)')) / 'Microsoft Visual Studio/Installer/vswhere.exe'
    installation = subprocess.check_output([str(vswhere), '-latest', '-prerelease', '-products', '*', '-requires',
                                           'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-property', 'installationPath'], text=True).strip()
    if not installation:
        raise RuntimeError('Visual C++ x64 build tools are required for the icon decoder')
    vcvars = Path(installation) / 'VC/Auxiliary/Build/vcvars64.bat'
    source = project / 'distribution/AssetRestore/native/icon_codec.c'
    command = f'@echo off\ncall "{vcvars}" >nul\nif errorlevel 1 exit /b %errorlevel%\ncl /nologo /O2 /MT /LD "{source}" /link /Brepro /OUT:Tower.IconCodec.dll\nexit /b %errorlevel%\n'
    (output / 'build-icon-codec.cmd').write_text(command, encoding='utf-8')
    subprocess.run(['cmd', '/d', '/c', 'build-icon-codec.cmd'], cwd=output, check=True)
    return output / 'Tower.IconCodec.dll'


def prepare(project, stage, game):
    project, stage, game = map(lambda p: Path(p).resolve(), (project, stage, game))
    targets = (*TARGETS, ENGLISH_THEME) if (stage / 'scene-assets' / ENGLISH_THEME).is_file() else TARGETS
    if stage == project or stage == game or stage == project / 'scene-assets':
        raise ValueError('Use a separate distribution stage')
    patches = stage / 'asset-patches'
    if patches.exists():
        raise ValueError('Asset patches already exist; use a fresh stage')
    validation = project / 'dist' / ('slim-validation-' + uuid.uuid4().hex)
    validation.mkdir(parents=True)
    codec = build_icon_codec(project, validation / 'helper')
    environment = dict(os.environ, TOWER_ICON_CODEC=str(codec))
    subprocess.run(['dotnet', 'run', '--project', str(project / 'scene-builder/SceneBuilder.csproj'),
                    '-c', 'Release', '--', str(game), str(patches), str(stage / 'scene-assets'), 'slim-assets'],
                   cwd=project, env=environment, check=True)
    subprocess.run(['dotnet', 'build', str(project / 'distribution/AssetRestore'), '-c', 'Release',
                    '-o', str(validation / 'helper')], check=True)
    shutil.copytree(patches, validation / 'asset-patches')
    helper = validation / 'helper/Tower.AssetRestore.exe'
    subprocess.run([str(helper), str(validation), str(game), '--build-validation'], check=True)
    manifest = json.loads((patches / 'manifest.json').read_text(encoding='utf-8'))
    if {f['Path'] for f in manifest['Files']} != set(targets):
        raise ValueError('Unexpected generated assets')
    for file in manifest['Files']:
        original = stage / 'scene-assets' / file['Path']
        rebuilt = validation / 'scene-assets' / file['Path']
        for path in (original, rebuilt):
            if hashlib.sha256(path.read_bytes()).hexdigest() != file['TargetHash']:
                raise ValueError(f'Asset reconstruction mismatch: {path}')
    icons = manifest.get('Icons') or []
    if len({i['Id'] for i in icons}) != len(icons) or any(type(i['Id']) is not int or not 0 <= i['Id'] < 990000 for i in icons):
        raise ValueError('Invalid native icon list')
    for icon in icons:
        for base in (stage, validation):
            path = base / 'scene-assets/reward-icons' / f"{icon['Id']}.rgba"
            if path.stat().st_size != 102400 or hashlib.sha256(path.read_bytes()).hexdigest() != icon['Sha256']:
                raise ValueError(f'Icon reconstruction mismatch: {path}')
    for name in ('Tower.AssetRestore.exe', 'Tower.AssetRestore.exe.config', 'Tower.IconCodec.dll'):
        shutil.copy2(validation / 'helper' / name, stage / name)
    (stage / 'licenses').mkdir(exist_ok=True)
    shutil.copy2(project / 'licenses/bcdec.txt', stage / 'licenses/bcdec.txt')
    for relative in targets:
        (stage / 'scene-assets' / relative).unlink()
    for relative in ('menu/low/01_common.tpf.dcx', 'menu/low/01_common.sblytbnd.dcx', 'locales/en-US/menu/low/01_common.tpf.dcx'):
        (stage / 'scene-assets' / relative).unlink(missing_ok=True)
    for icon in icons:
        (stage / 'scene-assets/reward-icons' / f"{icon['Id']}.rgba").unlink()
    (stage / 'asset-mode.txt').write_text('local-v1\n', encoding='ascii')
    print(f'Slim assets verified; reconstruction evidence kept at {validation}')


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('stage', type=Path)
    parser.add_argument('game', type=Path)
    args = parser.parse_args()
    prepare(Path(__file__).resolve().parents[1], args.stage, args.game)
