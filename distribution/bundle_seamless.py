"""Stage the verified Seamless runtime, never a player's settings or crash reports."""
import hashlib
import shutil
from pathlib import Path

SHA256 = '2ebde62be864ba3d759728aac44c459f0f813e8b6d7a342304ce68f6513f280c'


def stage_component(source, destination, template):
    source, destination = Path(source), Path(destination)
    dll = source / 'ersc.dll'
    if hashlib.sha256(dll.read_bytes()).hexdigest() != SHA256:
        raise ValueError('Expected pinned Seamless Co-op 2.0.3; component hash mismatch')
    files = [dll]
    for directory in ('locale', 'crashpad'):
        folder = source / directory
        if not folder.is_dir():
            raise ValueError(f'Missing Seamless component directory: {directory}')
        files.extend(p for p in folder.rglob('*') if p.is_file())
    for file in files:
        if file.is_symlink() or getattr(file.stat(), 'st_file_attributes', 0) & 0x400:
            raise ValueError(f'Reparse point in Seamless component: {file.name}')
        if file.suffix.lower() not in ('.dll', '.exe', '.json'):
            raise ValueError(f'Unexpected Seamless component file: {file.name}')
    for file in files:
        target = destination / file.relative_to(source)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(file, target)
    shutil.copy2(template, destination / 'default.ini')
