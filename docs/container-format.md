# Resource directory format

`PANIC.EXE` is a small DOS program with its resources appended after it,
followed by a directory. This describes the release listed in
[panic-release.json](../metadata/panic-release.json). Offsets are absolute file
positions, ranges are half-open (the end offset is excluded), and all integers
are little-endian.

| Range | Bytes | Contents |
|---|---:|---|
| `0x000000..0x006f91` | 28,561 | DOS MZ executable (size from its header) |
| `0x006f91..0x006f92` | 1 | Gap |
| `0x006f92..0x19d5ba` | 1,664,552 | Resource data, back to back |
| `0x19d5ba..0x19d5c6` | 12 | Directory header |
| `0x19d5c6..0x19d8ae` | 744 | 31 directory records |
| `0x19d8ae..0x19d8b2` | 4 | Offset of the directory header |

The last four bytes of the file hold `0x19d5ba`, the offset of the directory
header. The header is three unsigned 32-bit values: a marker (`0x00c82fc0`),
the record count (`31`) and the offset of the first resource (`0x6f92`). We
don't know what the marker means.

Each directory record is 24 bytes:

| Offset | Type | Meaning |
|---|---|---|
| 0 | 16 bytes | ASCII name, NUL-terminated and zero-padded |
| 16 | unsigned 32-bit | Absolute offset of the data |
| 20 | unsigned 32-bit | Length in bytes |

Resources are stored in directory order with no gaps or overlaps, and the last
one ends exactly where the directory header starts. Eight entries are `.EXE`
programs. The other 23 are a README, graphics data and the music. The names
are only a hint; this document doesn't describe the formats inside each file.

## How the tool validates a file

`panic_resources.py` first checks the file's size and SHA-256. It then checks
the MZ size, the directory location and record count, that every entry lies
within the file, that names are NUL-terminated, and that the entries are
contiguous and uniquely named.

To make extraction safe on common desktop filesystems, it also rejects names
containing path separators, names that differ only in case, and reserved
device names such as `CON`. That's a choice made by this tool, not something
the original format requires.

Extraction copies each resource unchanged and checks its hash. It doesn't
decode images or music, unpack the programs or run the loader; that's the job
of `import_panic.py`.

[panic-release.json](../metadata/panic-release.json) contains only names,
sizes, offsets and hashes. These were taken from the original release and
checked against it.
