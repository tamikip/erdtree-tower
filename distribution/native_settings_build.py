"""Pinned ERNativeUI host plus the tower's native configuration provider."""
from pathlib import Path
import hashlib
import os
import shutil
import subprocess
import urllib.request
import zipfile
import sys

VERSION = '1.3.0'
ARCHIVE = f'ERNativeUI-{VERSION}-windows-x64.zip'
SHA256 = '5f385b887ef0d0fe3bb0e7461fd8f5c8d9fe31f64dd5171df3bbc96fbd525e8d'


def build_native_settings(project, destination):
    subprocess.run([sys.executable, str(project / 'localization/generate_native_settings.py')], check=True)
    cache = project.parents[1] / 'work/tower-native-settings'
    cache.mkdir(parents=True, exist_ok=True)
    archive = cache / ARCHIVE
    if not archive.exists():
        try:
            urllib.request.urlretrieve(f'https://github.com/Flammrock/ERNativeUI/releases/download/ERNativeUI-v{VERSION}/{ARCHIVE}', archive)
        except OSError:
            subprocess.run(['gh', 'release', 'download', f'ERNativeUI-v{VERSION}', '--repo', 'Flammrock/ERNativeUI', '--pattern', ARCHIVE, '--dir', str(cache), '--clobber'], check=True)
    if hashlib.sha256(archive.read_bytes()).hexdigest() != SHA256:
        raise RuntimeError('ERNativeUI release checksum mismatch')
    sdk = cache / f'ERNativeUI-{VERSION}-windows-x64'
    with zipfile.ZipFile(archive) as bundle:
        for name in bundle.namelist():
            if not (cache / name).resolve().is_relative_to(cache.resolve()):
                raise RuntimeError('Unsafe ERNativeUI archive path')
        bundle.extractall(cache)
    cmake = os.environ.get('TOWER_CMAKE') or shutil.which('cmake')
    if not cmake:
        cmake = str(project.parents[1] / 'work/tower-transmog-build/cmake/cmake-4.4.5-windows-x86_64/bin/cmake.exe')
    build = cache / 'build'
    subprocess.run([cmake, '-S', str(project / 'native/native-settings'), '-B', str(build), '-A', 'x64', '-DCMAKE_PREFIX_PATH=' + str(sdk)], check=True)
    subprocess.run([cmake, '--build', str(build), '--config', 'Release', '--parallel', '4'], check=True)
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    shutil.copy2(build / 'Release/tower_native_settings.dll', destination)
    for name in ['ERNativeUI.dll', 'ERNativeUI.ini']:
        shutil.copy2(sdk / 'bin' / name, destination)
    shutil.copytree(sdk / 'bin/locales', destination / 'locales', dirs_exist_ok=True)
    for name in ['LICENSE.txt', 'THIRD_PARTY_NOTICES.txt']:
        shutil.copy2(sdk / 'share/ERNativeUI' / name, destination)
