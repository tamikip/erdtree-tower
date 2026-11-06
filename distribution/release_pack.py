"""Content-addressed update data for the launcher.

Every release uploads two GitHub Release assets:
  tower-pack-v<N>.bin   blobs (one per distinct file content) that no earlier release has, each raw-deflated or stored
  tower-release.json    the full file list of this version plus, for every file hash, where its blob lives
                        (release tag, pack asset, byte offset, length, method) — older blobs keep pointing at older packs

The launcher compares the latest tower-release.json with the hashes of the installed files and fetches only the missing blobs
with HTTP range requests (GitHub Release downloads answer 206), so skipped versions, partial installs and damaged files all
resolve to the same "download exactly what differs" plan.

Usage: python distribution/release_pack.py <stage dir> <version> <output dir> [--previous <previous tower-release.json>] [--notes <file>]
"""
import argparse
import hashlib
import json
import re
import zlib
from pathlib import Path

SCHEMA = 1
MANIFEST = 'tower-release.json'
# Files that belong to the player or to a running session and are never part of a release
PRIVATE = ('tower-install.json', 'GamePath.txt', 'tower-options.ini', 'preview.launch.me3')
PRIVATE_DIRS = ('.tower-update/', 'plugins/', 'config/', 'seamless-entry/')


def pack_name(version):
    return f'tower-pack-v{version}.bin'


def deflate(data):
    compressor = zlib.compressobj(9, zlib.DEFLATED, -15)
    packed = compressor.compress(data) + compressor.flush()
    return ('deflate', packed) if len(packed) < len(data) else ('store', data)


def collect(stage):
    files = []
    for path in sorted(p for p in stage.rglob('*') if p.is_file()):
        rel = path.relative_to(stage).as_posix()
        if rel in PRIVATE or rel.startswith(PRIVATE_DIRS):
            raise ValueError(f'Private file in release stage: {rel}')
        data = path.read_bytes()
        files.append({'path': rel, 'size': len(data), 'sha256': hashlib.sha256(data).hexdigest(), 'data': data})
    if not files:
        raise ValueError('Empty release stage')
    return files


def build(stage, version, output, previous=None, notes='', display_version='', launcher_version=''):
    if display_version and not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', display_version):
        raise ValueError('Display version must be major.minor.patch')
    if launcher_version and not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', launcher_version):
        raise ValueError('Launcher version must be major.minor.patch')
    stage, output = Path(stage), Path(output)
    output.mkdir(parents=True, exist_ok=True)
    tag = f'v{version}'
    known = {}
    if previous:
        old = json.loads(Path(previous).read_text(encoding='utf-8'))
        if old.get('schema') != SCHEMA:
            raise ValueError('Unsupported previous manifest schema')
        if int(old['version']) >= int(version):
            raise ValueError('Release version must increase')
        known = old['blobs']
    files = collect(stage)
    blobs, offset = {}, 0
    pack = output / pack_name(version)
    with pack.open('wb') as stream:
        for entry in files:
            sha = entry['sha256']
            if sha in blobs:
                continue
            if sha in known:
                blobs[sha] = known[sha]
                continue
            method, payload = deflate(entry['data'])
            stream.write(payload)
            blobs[sha] = {'tag': tag, 'asset': pack.name, 'offset': offset, 'length': len(payload), 'method': method, 'size': entry['size']}
            offset += len(payload)
    pack_bytes = pack.read_bytes()
    manifest = {
        'schema': SCHEMA,
        'version': int(version),
        'displayVersion': display_version,
        'launcherVersion': launcher_version,
        'updateKind': 'package',
        'tag': tag,
        'notes': notes,
        'files': [{'path': e['path'], 'size': e['size'], 'sha256': e['sha256']} for e in files],
        'blobs': dict(sorted(blobs.items())),
        'pack': {'asset': pack.name, 'size': len(pack_bytes), 'sha256': hashlib.sha256(pack_bytes).hexdigest()},
    }
    (output / MANIFEST).write_text(json.dumps(manifest, ensure_ascii=False, indent=1), encoding='utf-8')
    fresh = sum(1 for b in blobs.values() if b['tag'] == tag)
    print(f'{tag}: {len(files)} files, {len(blobs)} blobs ({fresh} new, {len(pack_bytes) / 1024 / 1024:.1f} MiB pack)')
    return manifest


def read_blob(pack_bytes, blob):
    """Decode one blob from its pack bytes (used by tests and by tools that verify an upload)."""
    raw = pack_bytes[blob['offset']:blob['offset'] + blob['length']]
    data = zlib.decompress(raw, -15) if blob['method'] == 'deflate' else raw
    if len(data) != blob['size']:
        raise ValueError('Blob size mismatch')
    return data


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('stage', type=Path)
    parser.add_argument('version', type=int)
    parser.add_argument('output', type=Path)
    parser.add_argument('--previous', type=Path)
    parser.add_argument('--notes', type=Path)
    parser.add_argument('--display-version', default='')
    args = parser.parse_args()
    build(args.stage, args.version, args.output, args.previous, args.notes.read_text(encoding='utf-8') if args.notes else '', args.display_version)
