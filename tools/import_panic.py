#!/usr/bin/env python3
"""Derive local Rust-player data from a user-supplied, hash-checked PANIC.EXE.

Fetches nothing and never runs a DOS executable on the host. Full import uses
Deark for embedded programs and the existing bounded Unicorn loader recovery.
Extraction does not grant permission to reuse or redistribute the material.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

from panic_resources import RELEASE_SHA256 as EXPECTED_SHA256, parse_directory, sha256 as sha

ROOT = Path(__file__).resolve().parents[1]
SOURCE_BYTES = 1693874
# Observed identities retained from the private research manifest and loader report.
# These are hashes, not embedded executable or asset bytes.
UNPACKED_SHA256 = {
    'PANIC.loader.exe': 'd34aa1db30f1372802acd413f4ba7cdfa080d616ee1d4c35179a2060e1367e45',
    '1.000.exe': '9d1763f0ec66a84ec9fefa728cd76639356a3308ee8820f78ca179715053530d',
    '2.000.exe': '96b86fa00989d50075be4b9c73fff7cd7d95d1584af9902b9c6fd8fd47978d8c',
    '3.000.exe': 'b779a89d01ac98416e5b7526d60d5cb57138975c99a537cd7534b3110a018821',
    '4.000.exe': '843a11e7a0f4a2106e445bc4e7cde457252e3efd2fdcf327cab1c377d5d7dc42',
    '5.final.000.exe': 'fb8daf80eb1905e6b2871aca333a800039c2b1c0379981a4cbd0aa7f8b09d1eb',
    '6.final.000.exe': '178d457f7883305d53d8f2be66c70764cf8f962ab1c8e018a36b58c093868706',
    '7.000.exe': 'd009f0d26c8c9ddcd986461cdc25c46292b1afd16861e173834b556fb68e419d',
    '8.000.exe': '5da80539e5ce30a575c41981c7fb194c1555dc868c738901ff5318547b215f03',
}


def read_source(source: Path) -> tuple[bytes, dict]:
    # A bounded read also rejects an unexpectedly large or changing input.
    with source.open('rb') as stream:
        data = stream.read(SOURCE_BYTES + 1)
    if len(data) != SOURCE_BYTES or sha(data) != EXPECTED_SHA256:
        raise ValueError('Unsupported PANIC.EXE: expected the unchanged 1,693,874-byte '
                         f'public release, SHA-256 {EXPECTED_SHA256}')
    return data, parse_directory(data)


def run_helper(command: list[str], cwd: Path, log: Path) -> None:
    with log.open('x') as stream:
        try:
            subprocess.run(command, cwd=cwd, stdout=stream, stderr=subprocess.STDOUT,
                           check=True, timeout=180)
        except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
            stream.flush()
            detail = log.read_text(errors='replace')[-3000:]
            raise ValueError(f'Import helper failed: {command[0]}\n{detail}') from error


def import_data(source: Path, destination: Path, *, assets_only: bool = False,
                deark: str = 'deark') -> dict:
    source = source.resolve(strict=True)
    # Check the unresolved path too: never follow an existing destination symlink.
    if destination.exists() or destination.is_symlink():
        raise ValueError('Destination already exists; choose a new directory')
    destination = destination.resolve()
    data, inventory = read_source(source)
    decoder = None
    if not assets_only:
        decoder = shutil.which(deark)
        if not decoder:
            raise ValueError('Deark not found; supply --deark /path/to/deark, or use '
                             '--assets-only for resources without player tables')
        decoder = str(Path(decoder).resolve())
        for module in ('unicorn', 'capstone'):
            if importlib.util.find_spec(module) is None:
                raise ValueError(f'Missing Python dependency {module}; run with the '
                                 'research virtualenv or install the documented dependencies')

    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.panic-import-', dir=destination.parent) as tmp:
        work = Path(tmp)
        output = work / 'data'
        files = output / 'files'
        files.mkdir(parents=True)
        packed = work / 'packed'
        packed.mkdir()
        entries = []
        for entry in inventory['entries']:
            payload = data[entry['offset']:entry['offset'] + entry['bytes']]
            is_executable = entry['name'].upper().endswith('.EXE')
            path = (packed if is_executable else files) / entry['name']
            path.write_bytes(payload)
            if not is_executable:
                entries.append({'path': f"files/{entry['name']}",
                                'bytes': entry['bytes'], 'sha256': entry['sha256'],
                                'parent': 'PANIC.EXE', 'offset': entry['offset'],
                                'operation': 'exact byte slice'})

        if not assets_only:
            unpacked = output / 'unpacked'
            unpacked.mkdir()
            logs = output / 'logs'
            logs.mkdir()
            for part in range(1, 9):
                run_helper([decoder, '-m', 'pklite', '-od', str(unpacked),
                            '-o', str(part), str(packed / f'{part}.EXE')],
                           work, logs / f'pklite-{part}.log')
                if part in (5, 6):
                    run_helper([decoder, '-m', 'exepack', '-od', str(unpacked),
                                '-o', f'{part}.final', str(unpacked / f'{part}.000.exe')],
                               work, logs / f'exepack-{part}.log')
                    (unpacked / f'{part}.000.exe').unlink()
            # Give the emulator an immutable copy of the bytes already checked.
            # Its memory, traces and intermediate stages stay in the scratch dir.
            carrier = work / 'PANIC.EXE'
            carrier.write_bytes(data)
            run_helper([sys.executable, str(ROOT / 'tools/unpack_panic.py'),
                        '--input', str(carrier), '--output', str(work / 'analysis')],
                       work, logs / 'loader.log')
            shutil.copyfile(work / 'derived/unpacked/PANIC.loader.exe',
                            unpacked / 'PANIC.loader.exe')
            recovery = json.loads((work / 'analysis/loader-emulation.json').read_text())
            (logs / 'loader-assumptions.json').write_text(json.dumps({
                'assumptions': recovery['assumptions'], 'stats': recovery['stats'],
                'error': recovery['error'], 'recovery': recovery['recovery'],
            }, indent=2) + '\n')
            if {p.name for p in unpacked.iterdir()} != set(UNPACKED_SHA256):
                raise ValueError('Unexpected unpacked file set')
            for name, expected in UNPACKED_SHA256.items():
                blob = (unpacked / name).read_bytes()
                if sha(blob) != expected:
                    raise ValueError(f'Unpacked hash mismatch for {name}; use the '
                                     'documented Deark revision and loader dependencies')
                entries.append({'path': f'unpacked/{name}', 'bytes': len(blob),
                                'sha256': expected, 'parent': 'PANIC.EXE',
                                'operation': 'bounded loader emulation' if name.startswith('PANIC')
                                else 'Deark PKLITE/EXEPACK decoding'})

        # Preserve the embedded original terms in files/README.TXT. A local
        # ignore file also helps when the user selects a destination outside artifacts/.
        (output / '.gitignore').write_text('*\n!.gitignore\n')
        report = {
            'schema_version': 1,
            'source': {'name': 'PANIC.EXE', 'bytes': len(data), 'sha256': sha(data)},
            'mode': 'assets-only' if assets_only else 'rust-player',
            'resource_count': sum(e['path'].startswith('files/') for e in entries),
            'unpacked_count': sum(e['path'].startswith('unpacked/') for e in entries),
            'files': sorted(entries, key=lambda e: e['path']),
            'rights': 'Original terms are retained in files/README.TXT. Local extraction '
                      'does not grant rights to reuse or redistribute the demo material.',
            'limitations': [
                'Only the pinned public release is supported.',
                'Unpacked modules contain original code as well as tables; they remain local data.',
                'This import does not establish legal clearance or complete reverse engineering.',
            ],
        }
        # Read every final payload back, including exact resource slices.
        for entry in entries:
            blob = (output / entry['path']).read_bytes()
            if (len(blob), sha(blob)) != (entry['bytes'], entry['sha256']):
                raise ValueError(f"Output verification failed: {entry['path']}")
        (output / 'import.json').write_text(json.dumps(report, indent=2) + '\n')
        # Claim the new directory exclusively after all checks have passed.
        # Unlike rename on POSIX, mkdir cannot replace an existing empty directory.
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
    parser.add_argument('source', type=Path, help='Your unchanged PANIC.EXE')
    parser.add_argument('--out', type=Path, default=ROOT / 'artifacts/local-data')
    parser.add_argument('--deark', default='deark', help='Deark executable path (full import)')
    parser.add_argument('--assets-only', action='store_true',
                        help='Extract 23 resources only; insufficient to run the Rust player')
    args = parser.parse_args()
    try:
        report = import_data(args.source, args.out, assets_only=args.assets_only, deark=args.deark)
    except (OSError, ValueError) as error:
        parser.exit(1, f'Import failed: {error}\n')
    print(json.dumps({'output': str(args.out.resolve()), 'mode': report['mode'],
                      'resources': report['resource_count'],
                      'unpacked_modules': report['unpacked_count']}, indent=2))


if __name__ == '__main__':
    main()
