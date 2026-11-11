"""Check published scene fingerprints and require committed release inputs."""
from pathlib import Path
import argparse
import hashlib
import json
import subprocess

INPUTS = ('src', 'native', 'scene-builder', 'scene-assets', 'distribution', 'launcher', 'tests', 'localization',
          'Cargo.toml', 'Cargo.lock', 'release-preview/preview.me3', 'Launch-Preview.ps1',
          'Launch-Seamless-Tower.ps1', 'seamless-preset.ini', 'THIRD_PARTY_NOTICES.md',
          'SeamlessCoop/default.ini', 'SeamlessCoop/README.md', 'licenses', 'loader/yafsml')


def require_committed(root):
    def git(*args):
        return subprocess.check_output(['git', *args], cwd=root).decode('utf-8').split('\0')
    changed = git('diff', '--name-only', '-z', 'HEAD', '--', *INPUTS)
    changed += git('ls-files', '--others', '--exclude-standard', '-z', '--', *INPUTS)
    changed = sorted(set(changed) - {''})
    if changed:
        raise ValueError('Release source/generated assets must be committed first:\n' + '\n'.join(changed))


def normalized_hash(data):
    if b'\0' in data:
        return None
    try:
        data.decode('utf-8-sig')
    except UnicodeDecodeError:
        return None
    return hashlib.sha256(data.replace(b'\r\n', b'\n')).hexdigest()


def audit(root, baseline, deviations):
    allowed = {item['path']: item for item in deviations}
    checked = exact = normalized = 0
    exceptions = []
    for entry in baseline['files']:
        relative = entry['path']
        path = (root / relative).resolve()
        if root.resolve() not in path.parents:
            raise ValueError('Invalid manifest path: ' + relative)
        data = path.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        checked += 1
        if len(data) == entry['size'] and digest == entry['sha256']:
            exact += 1
        elif entry.get('lfSha256') and normalized_hash(data) == entry['lfSha256']:
            normalized += 1
        elif relative in allowed and digest == allowed[relative]['sha256']:
            exceptions.append(relative)
        else:
            raise ValueError('Unexplained release resource mismatch: ' + relative)
    if set(exceptions) != set(allowed):
        raise ValueError('Unused or stale deviation declarations')
    for line in (root / 'scene-assets/arenas-resources.txt').read_text(encoding='utf-8-sig').splitlines():
        if not line.strip():
            continue
        relative, size, expected = line.split('|')
        path = (root / 'scene-assets' / relative).resolve()
        if (root / 'scene-assets').resolve() not in path.parents:
            raise ValueError('Invalid arena path: ' + relative)
        data = path.read_bytes()
        value = 14695981039346656037
        for byte in data:
            value = ((value ^ byte) * 1099511628211) & ((1 << 64) - 1)
        if len(data) != int(size) or value != int(expected, 16):
            raise ValueError('Arena fingerprint mismatch: ' + relative)
    return {'checked': checked, 'exact': exact, 'newlineOnly': normalized, 'documentedDifferences': exceptions}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--baseline', type=Path)
    parser.add_argument('--differences', type=Path)
    parser.add_argument('--check-git', action='store_true')
    args = parser.parse_args()
    if args.check_git:
        require_committed(args.root)
    if args.baseline:
        differences = json.loads(args.differences.read_text(encoding='utf-8')) if args.differences else []
        print(json.dumps(audit(args.root, json.loads(args.baseline.read_text(encoding='utf-8')), differences), ensure_ascii=False, indent=2))
