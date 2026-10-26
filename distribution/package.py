from pathlib import Path
import hashlib
import shutil
import zipfile
import re
from native_build import build_native

project = Path(__file__).resolve().parents[1]
source = project / 'release-preview'
dist = project / 'dist'
profile = (source / 'preview.me3').read_text(encoding='utf-8-sig')
native = re.findall(r"path\s*=\s*['\"](erdtree_tower_preview_v\d+\.dll)['\"]", profile)
if len(native) != 1:
    raise SystemExit('Expected one versioned native DLL in preview.me3')
version = re.search(r'_v(\d+)\.dll$', native[0]).group(1)
stage = dist / f'ErdtreeTower-v{version}'
if stage.exists():
    raise SystemExit('Staging directory already exists; preserve it or choose a new release directory.')
built_native = build_native(project)
stage.mkdir(parents=True)

def copy_file(src, relative):
    target = stage / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, target)

def copy_tree(src, relative):
    for item in src.rglob('*'):
        if item.is_symlink() or item.stat().st_file_attributes & 0x400:
            raise RuntimeError(f'Reparse point rejected: {item}')
        if item.is_file():
            copy_file(item, Path(relative) / item.relative_to(src))

copy_file(built_native, native[0])
for name in ['preview.me3', 'Launch-Preview.ps1', 'THIRD_PARTY_NOTICES.md', 'LICENSE', 'ADDITIONAL-TERMS.md']:
    copy_file(source / name, name)
# Ship launcher sources directly so a stale release-preview copy cannot leak into the package.
for name in ['Launch-Seamless-Tower.ps1', 'New-PartyConfig.ps1']:
    copy_file(project / name, name)
for name in ['scene-assets', 'licenses']:
    copy_tree(project / name, name)
copy_tree(source / 'loader/bin', 'loader/bin')
for name in ['LICENSE-MIT', 'LICENSE-APACHE']:
    copy_file(source / 'loader' / name, 'loader/' + name)
copy_file(source / 'seamless-entry/dinput8.dll', 'seamless-entry/dinput8.dll')
engine = project.parents[1] / 'work/seamless84-me2/ModEngine-2.1.0.0-win64/modengine2'
copy_tree(engine / 'bin', 'seamless-entry/modengine2/bin')
copy_tree(engine / 'assets', 'seamless-entry/modengine2/assets')
copy_tree(engine / 'crashpad', 'seamless-entry/modengine2/crashpad')
copy_file(project.parents[1] / 'work/modengine2-package-LICENSE.txt', 'licenses/ModEngine2-MIT.txt')
for name in ['Start-Tower.ps1', '安装教程.md']:
    copy_file(Path(__file__).parent / name, name)
copy_file(project / 'reference/联机战斗验证.md', '联机战斗验证.md')
for mode in ['Solo', 'Seamless']:
    (stage / f'Start-{mode}.cmd').write_text(
        '@echo off\nsetlocal\n'
        '"%SystemRoot%\\System32\\WindowsPowerShell\\v1.0\\powershell.exe" '
        f'-NoProfile -ExecutionPolicy Bypass -File "%~dp0Start-Tower.ps1" -Mode {mode} %*\n'
        'set "tower_exit_code=%errorlevel%"\nif not "%tower_exit_code%"=="0" pause\n'
        'exit /b %tower_exit_code%\n', encoding='ascii')
(stage / '安装教程.txt').write_text((stage / '安装教程.md').read_text(encoding='utf-8'), encoding='utf-8-sig')
with (stage / 'THIRD_PARTY_NOTICES.md').open('a', encoding='utf-8') as stream:
    stream.write('\nMod Engine 2 2.1.0: https://github.com/soulsmods/ModEngine2 ; '
                 'MIT notice: licenses/ModEngine2-MIT.txt. Seamless Co-op is not bundled.\n')
files = sorted(p for p in stage.rglob('*') if p.is_file())
for file in files:
    name = file.relative_to(stage).as_posix()
    if (file.suffix in ['.log', '.sl2', '.co2', '.ertowercoop', '.ertowerparty', '.party-award'] or
            file.name in ['host.json', 'guest.json', 'GamePath.txt'] or
            file.name.startswith('tower-') or 'settings' in file.name or '/runtime/' in name or '/party-runtime/' in name):
        raise RuntimeError(f'Private runtime file rejected: {name}')
manifest = ''.join(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.relative_to(stage).as_posix()}\n' for p in files)
(stage / 'SHA256SUMS.txt').write_text(manifest, encoding='utf-8')
archive = dist / f'ErdtreeTower-v{version}.zip'
with zipfile.ZipFile(archive, 'x', zipfile.ZIP_DEFLATED, compresslevel=6) as output:
    for file in sorted(stage.rglob('*')):
        if file.is_file():
            output.write(file, Path(stage.name) / file.relative_to(stage))
with zipfile.ZipFile(archive) as output:
    assert output.testzip() is None
checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
(dist / f'ErdtreeTower-v{version}.zip.sha256').write_text(f'{checksum}  {archive.name}\n', encoding='ascii')
print(f'Packaged {len(files) + 1} files, {archive.stat().st_size / 1024**2:.1f} MiB: {archive}')
print(f'SHA256 {checksum}')
