from pathlib import Path
import argparse
import hashlib
import json
import shutil
import subprocess


def build_native(project):
    project = Path(project).resolve()
    manifest = project / 'native/Cargo.toml'
    metadata = subprocess.run([
        'cargo', 'metadata', '--offline', '--locked', '--no-deps', '--format-version', '1',
        '--manifest-path', str(manifest),
    ], check=True, capture_output=True, text=True)
    package = next(p for p in json.loads(metadata.stdout)['packages']
                   if p['name'] == 'erdtree-tower-preview')
    dependency = next(d for d in package['dependencies'] if d['name'] == 'hudhook')
    if (dependency['source'] is not None or not dependency.get('path')
            or Path(dependency['path']).resolve() != project / 'native/vendor/hudhook'
            or 'dx12' not in dependency['features']):
        raise RuntimeError('Packaging requires the vendored DX12-fixed hudhook dependency.')
    target = project.parents[1] / 'work/erdtree-native-build'
    subprocess.run([
        'cargo', 'build', '--offline', '--locked', '--release', '--lib',
        '--manifest-path', str(manifest), '--target-dir', str(target),
    ], check=True)
    native = target / 'release/erdtree_tower_preview.dll'
    if b'DX12 queue captured from submitted back-buffer PRESENT transition' not in native.read_bytes():
        raise RuntimeError('Built DLL is missing the DX12 back-buffer queue-discovery fix.')
    return native


def stage_native(project, destination, checksum=None):
    native = build_native(project)
    destination = Path(destination)
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(native, destination)
    digest = hashlib.sha256(destination.read_bytes()).hexdigest()
    if checksum is not None:
        Path(checksum).write_text(digest + '\n', encoding='ascii')
    return digest


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--destination', type=Path, required=True)
    parser.add_argument('--checksum', type=Path)
    args = parser.parse_args()
    print(stage_native(Path(__file__).resolve().parents[1], args.destination, args.checksum))
