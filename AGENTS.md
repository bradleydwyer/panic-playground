# Panic playground

This is an experiment with modern rendering of Panic's intro through the logo,
built on the Rust port. Preserve the original sequence and music cues while
developing the modern renderer. Don't present authored geometry or motion as
recovered original models or equations.

This repository is public, including the Rust port and the data importer, at
the owner's request. Don't change its visibility unless asked. Its history is
separate from the private research project; don't merge that history in.

Being public doesn't mean the port is cleared for copyright. Keep all of the
following out of Git: original or extracted demo files, disassembly,
decompiler output, binary fixtures, screenshots, recordings, fonts, samples and
the locations of private archives. There are no binary asset exceptions in
this repository. Player data is generated locally from a
`PANIC.EXE` the user supplies.

Default tests must use synthetic data. Python tests that need the original
release are enabled with `PANIC_EXE`. Rust tests that need original data are
`#[ignore]`d and read `PANIC_DATA` and `PANIC_FIXTURES`. Never modify files the
user passes in. Write local output under the ignored `artifacts/` directory.
Never execute DOS code on the host.

In code comments and docs, keep clear what was read from the bytes, what is
interpretation, what was observed at run time and what is still a guess.
Don't claim the reverse engineering is complete.

For code changes, run:

```sh
python3 -m unittest discover -s tests -v
cargo test --release --locked --no-default-features
cargo test --release --locked --features hires
```

After staging, run `python3 tools/check_publication.py --research`. (Without
`--research` it checks the smaller tooling-only scope.) Any change to the file
lists in that script needs review. It only checks file scope and text; it says
nothing about legal clearance.
