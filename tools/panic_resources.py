#!/usr/bin/env python3
"""Inspect or extract Panic's appended resources without executing DOS code."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import struct
import tempfile

ROOT = Path(__file__).resolve().parents[1]
RELEASE_BYTES = 1_693_874
RELEASE_SHA256 = 'a7ceeae290a7132880cd6ba616c2b0bb4001b2f7c83694f04052314fa26df6e7'
RESERVED_NAMES = {'CON', 'PRN', 'AUX', 'NUL', 'CONIN$', 'CONOUT$'} | {
    f'{prefix}{number}' for prefix in ('COM', 'LPT') for number in range(1, 10)
}


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def parse_directory(data: bytes) -> dict:
    """Validate the documented container layout; suitable for synthetic inputs.

    The directory marker is recorded as an opaque value. Parsing establishes
    byte layout, not the meaning of payloads or the behaviour of the demo.
    """
    if len(data) < 44 or data[:2] != b'MZ':
        raise ValueError('Missing or truncated MZ carrier')
    last, pages, _, paragraphs = struct.unpack_from('<4H', data, 2)
    if pages == 0 or last > 511:
        raise ValueError('Invalid MZ page count or final-page size')
    module_end = (pages - 1) * 512 + (last or 512)
    header_end = paragraphs * 16
    if not 28 <= header_end <= module_end <= len(data):
        raise ValueError('MZ header/module extent outside carrier')

    directory = struct.unpack_from('<I', data, len(data) - 4)[0]
    if not module_end <= directory <= len(data) - 16:
        raise ValueError('Directory pointer outside appended region')
    marker, count, start = struct.unpack_from('<III', data, directory)
    if not 1 <= count <= 4096:
        raise ValueError('Invalid resource count')
    if directory + 12 + count * 24 != len(data) - 4:
        raise ValueError('Directory extent does not end at footer')
    if not module_end <= start <= directory:
        raise ValueError('Resource start outside appended region')

    entries = []
    next_offset = start
    seen = set()
    for index in range(count):
        record = directory + 12 + index * 24
        raw_name, offset, size = struct.unpack_from('<16sII', data, record)
        name, separator, padding = raw_name.partition(b'\0')
        if not separator or padding.strip(b'\0'):
            raise ValueError('Invalid resource name termination or padding')
        if not re.fullmatch(rb'[A-Za-z0-9][A-Za-z0-9_.-]*', name):
            raise ValueError('Unsafe resource name')
        name = name.decode('ascii')
        canonical = name.upper()
        if name.endswith('.') or canonical.split('.')[0] in RESERVED_NAMES:
            raise ValueError('Unsafe platform-specific resource name')
        if canonical in seen:
            raise ValueError('Duplicate resource name, ignoring case')
        seen.add(canonical)
        if offset != next_offset or offset + size > directory:
            raise ValueError('Resource gap, overlap or out-of-bounds extent')
        entries.append({
            'index': index, 'name': name, 'offset': offset, 'bytes': size,
            'sha256': sha256(data[offset:offset + size]),
        })
        next_offset = offset + size
    if next_offset != directory:
        raise ValueError('Unaccounted payload bytes before directory')
    return {
        'schema_version': 1,
        'source': {'bytes': len(data), 'sha256': sha256(data)},
        'directory': {'offset': directory, 'bytes': 12 + count * 24,
                      'marker': marker, 'first_resource_offset': start},
        'entries': entries,
    }


def read_release(source: Path) -> bytes:
    """Read only the supported release, with a bounded read and identity check."""
    with source.open('rb') as stream:
        data = stream.read(RELEASE_BYTES + 1)
    if len(data) != RELEASE_BYTES or sha256(data) != RELEASE_SHA256:
        raise ValueError(f'Unsupported release: expected {RELEASE_BYTES:,} bytes '
                         f'and SHA-256 {RELEASE_SHA256}')
    return data


def extract_resources(data: bytes, destination: Path, *, include_programs: bool = False) -> dict:
    """Extract validated slices into a new directory; never overwrite inputs.

    This format-level function accepts synthetic containers for tests. The CLI
    additionally verifies the known public release through read_release().
    """
    inventory = parse_directory(data)
    if destination.exists() or destination.is_symlink():
        raise ValueError('Destination already exists; choose a new directory')
    destination = destination.resolve()
    selected = [entry for entry in inventory['entries']
                if include_programs or not entry['name'].upper().endswith('.EXE')]
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.panic-extract-', dir=destination.parent) as tmp:
        output = Path(tmp)
        files = output / 'files'
        files.mkdir()
        for entry in selected:
            payload = data[entry['offset']:entry['offset'] + entry['bytes']]
            target = files / entry['name']
            target.write_bytes(payload)
            if sha256(target.read_bytes()) != entry['sha256']:
                raise ValueError(f"Output verification failed: {entry['name']}")
        report = {
            **inventory,
            'extracted': [entry['name'] for entry in selected],
            'include_programs': include_programs,
            'rights': 'Extracted material retains its original rights. See the embedded '
                      'files/README.TXT when using the supported release. This tool '
                      'grants no permission to reuse or redistribute that material.',
            'limitations': 'Exact resource slices only. No decompression or demo execution; '
                           'these files alone cannot run the research player.',
        }
        (output / 'inventory.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
        (output / '.gitignore').write_text('*\n!.gitignore\n', encoding='utf-8')
        # mkdir exclusively claims the destination after extraction has passed.
        # Do not use rename, which can replace an existing empty directory on POSIX.
        destination.mkdir()
        try:
            for path in output.iterdir():
                shutil.move(str(path), destination / path.name)
        except BaseException:
            shutil.rmtree(destination)
            raise
    return report


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    inspect = commands.add_parser('inspect', help='Print directory metadata and hashes as JSON')
    inspect.add_argument('source', type=Path)
    extract = commands.add_parser('extract', help='Extract local resources to a new directory')
    extract.add_argument('source', type=Path)
    extract.add_argument('--out', type=Path, default=ROOT / 'artifacts/local-data')
    extract.add_argument('--include-programs', action='store_true',
                         help='Also copy the eight packed embedded EXEs unchanged; never run them')
    args = parser.parse_args()
    try:
        data = read_release(args.source)
        if args.command == 'inspect':
            result = parse_directory(data)
        else:
            report = extract_resources(data, args.out, include_programs=args.include_programs)
            result = {'output': str(args.out.resolve()), 'resources': len(report['extracted']),
                      'source_sha256': report['source']['sha256'],
                      'inventory': str(args.out.resolve() / 'inventory.json')}
    except (OSError, ValueError) as error:
        parser.exit(1, f'Panic resource check failed: {error}\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
