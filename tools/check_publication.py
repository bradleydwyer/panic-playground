#!/usr/bin/env python3
"""Check the staged snapshot and reachable history against the selected scope.

This checks file boundaries and text constraints, not copyright ownership.
Review additions to APPROVED_FILES and the contents themselves before release.
"""
from pathlib import Path
import argparse
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
APPROVED_FILES = {
    '.gitignore', 'AGENTS.md', 'LICENSE', 'NOTICE', 'README.md',
    'docs/container-format.md', 'docs/publication.md', 'metadata/panic-release.json',
    'tests/test_resources.py', 'tests/test_local_release.py',
    'tools/panic_resources.py', 'tools/check_publication.py',
}
RESEARCH_FILES = {
    '.cargo/config.toml', 'Cargo.toml', 'Cargo.lock', 'requirements-import.txt',
    'rust/Cargo.toml', 'rust/README.md', 'rust/NOTICE',
    'rust/src/data.rs', 'rust/src/engine.rs', 'rust/src/lib.rs',
    'rust/src/main.rs', 'rust/src/music.rs', 'rust/src/vga.rs',
    'rust/src/test_support.rs',
    'rust/src/bin/panic-hd.rs', 'rust/src/hd/mod.rs',
    'rust/src/hd/clip.rs', 'rust/src/hd/renderer.rs', 'rust/src/hd/modern.rs',
    'rust/src/hd/intro.wgsl', 'rust/src/hd/modern.wgsl', 'rust/src/hd/post.wgsl',
    'rust/src/hd/transition.wgsl',
    'rust/src/hd/scroller.wgsl', 'rust/src/hd/wall.wgsl',
    # Part 6: its state and fields, its GPU side, the shared shader code,
    # faithful HD and the style (each style edits only its own file).
    'rust/src/hd/rings.rs', 'rust/src/hd/rings_render.rs',
    'rust/src/hd/rings_common.wgsl', 'rust/src/hd/rings_style.wgsl',
    'rust/src/hd/rings_faithful.wgsl', 'rust/src/hd/rings_pool.wgsl',
    'rust/src/hd/rings_symbols.txt', 'rust/src/hd/rings_authored.rs',
    # Part 5: its state, fit and relief, its GPU side, the shared shader code,
    # faithful HD, the entry points and the three authors' files (each author
    # edits only their own WGSL and Rust files).
    'rust/src/hd/field.rs', 'rust/src/hd/field_render.rs',
    'rust/src/hd/field_common.wgsl', 'rust/src/hd/field_style.wgsl',
    'rust/src/hd/field_faithful.wgsl',
    'rust/src/hd/field_frame.wgsl', 'rust/src/hd/field_frame.rs',
    # The frame author's smoke: its host code and its compute kernels
    # (authored, lifted from a local prototype; no demo data).
    'rust/src/hd/field_frame_smoke.rs',
    'rust/src/hd/field_frame_smoke/solver3d.wgsl',
    'rust/src/hd/field_frame_smoke/multigrid.wgsl',
    'rust/src/hd/field_frame_smoke/light3d.wgsl',
    'rust/src/hd/field_frame_smoke/solver2d.wgsl',
    'rust/src/hd/field_frame_smoke/light2d.wgsl',
    'rust/src/hd/field_frame_smoke/terrain.wgsl',
    'rust/src/hd/field_zoom.wgsl', 'rust/src/hd/field_zoom.rs',
    'rust/src/hd/field_mountain.wgsl', 'rust/src/hd/field_mountain.rs',
    # Part 3: its state and tables, its GPU side, the shared shader code,
    # faithful HD and the modern light painting.
    'rust/src/hd/dots.rs', 'rust/src/hd/dots_render.rs',
    'rust/src/hd/dots_common.wgsl', 'rust/src/hd/dots_faithful.wgsl',
    'rust/src/hd/dots_light.wgsl',
    # Part 4: its state and tables, the slab fitted to the logo, its GPU
    # side, the shared shader code, faithful HD and the modern light on
    # stone.
    'rust/src/hd/plasma.rs', 'rust/src/hd/plasma_stone.rs',
    'rust/src/hd/plasma_render.rs', 'rust/src/hd/plasma_common.wgsl',
    'rust/src/hd/plasma_faithful.wgsl', 'rust/src/hd/plasma_light.wgsl',
    # Part 7: its state and tables, the letterforms derived from the user's
    # data at load (no glyph data), its GPU side, the shared shader code,
    # faithful HD, the modern lighthouse and the authored letters' outlines.
    'rust/src/hd/greetings.rs', 'rust/src/hd/greetings_type.rs',
    'rust/src/hd/greetings_render.rs', 'rust/src/hd/greetings_common.wgsl',
    'rust/src/hd/greetings_faithful.wgsl', 'rust/src/hd/greetings_light.wgsl',
    'rust/src/hd/greetings_letters.txt',
    'rust/src/scenes/mod.rs', 'rust/src/scenes/intro.rs',
    'rust/src/scenes/scroller.rs', 'rust/src/scenes/field.rs',
    'rust/src/scenes/rings.rs', 'rust/src/scenes/dots.rs',
    'rust/src/scenes/plasma.rs', 'rust/src/scenes/greetings.rs',
    'rust/src/scenes/ending.rs',
    'tools/import_panic.py', 'tools/unpack_panic.py', 'tests/test_import_panic.py',
    'tools/look_check.py', 'tests/test_look_check.py',
    # Part 6's authored symbols (not demo data): the drawings, their kit and
    # previewer, and the exporter that writes rust/src/hd/rings_symbols.txt.
    'tools/symbols/neon.py', 'tools/symbols/render.py',
    'tools/symbols/pattern_1.py', 'tools/symbols/pattern_2.py',
    'tools/symbols/pattern_3.py', 'tools/symbols/pattern_4.py',
    'tools/symbols/pattern_5.py', 'tools/symbols/pattern_6.py',
    'tools/symbols/pattern_7.py', 'tools/symbols/export_symbols.py',
    'tests/test_symbols.py',
    # Part 7's authored letters (not demo data): the drawings, their kit
    # and previewer, and the exporter that writes
    # rust/src/hd/greetings_letters.txt.
    'tools/letters/kit.py', 'tools/letters/glyphs.py', 'tools/letters/render.py',
    'tools/letters/export_letters.py', 'tests/test_letters.py',
}
# Research files since removed from the tree: earlier commits may hold them,
# the index may not. Part 6's other four styles, tried and dropped.
RETIRED_FILES = {
    'rust/src/hd/rings_plus.wgsl', 'rust/src/hd/rings_glass.wgsl',
    'rust/src/hd/rings_room.wgsl', 'rust/src/hd/rings_volume.wgsl',
}


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT)


def check_rows(records, label, allowed, *, staged=False):
    names = set()
    blobs = []
    for record in records.split(b'\0'):
        if not record:
            continue
        fields, name = record.split(b'\t', 1)
        name = name.decode('utf-8')
        mode, second, third = fields.decode('ascii').split()
        if mode not in ('100644', '100755'):
            raise ValueError(f'{label}: symlink or non-file entry: {name}')
        if staged:
            if third != '0':
                raise ValueError(f'{label}: unresolved index entry: {name}')
            oid = second
        else:
            if second != 'blob':
                raise ValueError(f'{label}: unexpected object type: {name}')
            oid = third
        if name not in allowed:
            if staged and name in RETIRED_FILES:
                raise ValueError(f'{label}: retired file staged again: {name}')
            if name in RESEARCH_FILES or name in RETIRED_FILES:
                raise ValueError(f'{label}: research file outside standalone-tooling scope: {name}. '
                                 'Use --research to check the complete research source scope.')
            raise ValueError(f'{label}: file outside reviewed scope: {name}')
        names.add(name)
        blobs.append((name, oid))
    if staged and names != allowed:
        raise ValueError(f'Missing staged files: {sorted(allowed - names)}')
    return blobs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--research', '--private', dest='research', action='store_true',
                        help='Include the explicit research source list; not publication clearance')
    args = parser.parse_args()
    allowed = APPROVED_FILES | RESEARCH_FILES if args.research else APPROVED_FILES
    history = allowed | RETIRED_FILES if args.research else allowed
    try:
        blobs = check_rows(git('ls-files', '--stage', '-z'), 'index', allowed, staged=True)
        commits = git('rev-list', '--all').decode('ascii').splitlines()
        for commit in commits:
            blobs.extend(check_rows(git('ls-tree', '-rz', '--full-tree', commit), commit, history))
        seen = set()
        for name, oid in blobs:
            if oid in seen:
                continue
            seen.add(oid)
            if int(git('cat-file', '-s', oid)) > 128 * 1024:
                raise ValueError(f'Unexpectedly large text file: {name}')
            content = git('cat-file', 'blob', oid).decode('utf-8')
            if '\0' in content:
                raise ValueError(f'Binary/NUL content: {name}')
            # Split literals so this checker does not match its own source.
            forbidden = ['/' + 'Users/', '/' + 'home/', 'research' + '-private',
                         'divoom-minitoo' + '-research', 'gh' + 'p_']
            if any(marker in content for marker in forbidden):
                raise ValueError(f'Local/private-path or token marker: {name}')
        scope = 'research source' if args.research else 'standalone tooling'
        print(f'Checked {len(allowed)} staged files, {len(commits)} commits, '
              f'{len(seen)} distinct text blobs against {scope} scope. '
              'This is not legal clearance.')
    except (OSError, ValueError, UnicodeError, subprocess.CalledProcessError) as error:
        print(f'Publication check failed: {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
