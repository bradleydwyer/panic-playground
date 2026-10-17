//! Local-only oracle inputs and synthetic tests. No original data is embedded.
use crate::data::Data;
use std::{env, fs, path::PathBuf};

fn configured_root(variable: &str, description: &str) -> PathBuf {
    env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("Set {variable} to {description} before running ignored tests"))
}

pub fn data() -> Data {
    let root = configured_root(
        "PANIC_DATA",
        "a local imported data directory containing files/ and unpacked/, or the research root",
    );
    Data::new(&root)
        .unwrap_or_else(|error| panic!("Invalid PANIC_DATA at {}: {error}", root.display()))
}

pub fn fixture(relative: &str) -> Vec<u8> {
    let root = configured_root(
        "PANIC_FIXTURES",
        "the local research rust/ directory containing tests/fixtures/ and testdata/567/",
    );
    let path = root.join(relative);
    fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "Cannot read PANIC_FIXTURES input {}: {error}. Oracle fixtures are not bundled.",
            path.display()
        )
    })
}

pub fn fixture_text(relative: &str) -> String {
    String::from_utf8(fixture(relative))
        .unwrap_or_else(|error| panic!("PANIC_FIXTURES input {relative} is not UTF-8: {error}"))
}

#[cfg(test)]
mod synthetic {
    use super::*;
    use crate::{
        data::{signed, word, words},
        vga::Vga,
    };
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            loop {
                let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
                let path =
                    env::temp_dir().join(format!("panic-rust-test-{}-{id}", std::process::id()));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("Cannot create synthetic test directory: {error}"),
                }
            }
        }

        fn player_data(&self) -> Data {
            fs::create_dir_all(self.0.join("files")).unwrap();
            fs::create_dir_all(self.0.join("unpacked")).unwrap();
            fs::write(self.0.join("files/P.P"), b"invented test marker").unwrap();
            Data::new(&self.0).unwrap()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn put_word(bytes: &mut [u8], at: usize, value: u16) {
        bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn invented_mz() -> Vec<u8> {
        // A 48-byte MZ header followed by 16 bytes of invented data, not code.
        let mut bytes = vec![0; 64];
        for (index, value) in [0x5a4d, 64, 1, 2, 3, 0, 0xffff, 0, 0, 0, 0, 0, 0x1c, 0]
            .into_iter()
            .enumerate()
        {
            put_word(&mut bytes, index * 2, value);
        }
        put_word(&mut bytes, 0x1c, 2);
        put_word(&mut bytes, 0x20, 8);
        put_word(&mut bytes, 48, 0xabcd);
        put_word(&mut bytes, 50, 0xfff8);
        put_word(&mut bytes, 56, 0x0110);
        bytes
    }

    #[test]
    fn portable_and_research_layouts_read_only_the_selected_files() {
        let portable = Scratch::new();
        let data = portable.player_data();
        fs::write(portable.0.join("files/INVENTED.DAT"), [4, 9, 16]).unwrap();
        assert_eq!(data.file("INVENTED.DAT").unwrap(), [4, 9, 16]);
        assert!(data.file("ABSENT.DAT").unwrap_err().contains("ABSENT.DAT"));

        let project = Scratch::new();
        let files = project.0.join("artifacts/derived/assets/files");
        fs::create_dir_all(&files).unwrap();
        fs::write(files.join("P.P"), b"another invented marker").unwrap();
        assert_eq!(
            Data::new(&project.0).unwrap().file("P.P").unwrap(),
            b"another invented marker"
        );
        assert!(Data::new(project.0.join("missing")).is_err());
    }

    #[test]
    fn mz_loading_relocates_words_with_wraparound_and_zero_fills_memory() {
        let scratch = Scratch::new();
        let data = scratch.player_data();
        let path = scratch.0.join("unpacked/1.000.exe");
        let source = invented_mz();
        fs::write(&path, &source).unwrap();
        let memory = data.memory(1).unwrap();
        assert_eq!(memory.len(), 0x110000);
        assert_eq!(word(&memory, 0x10000), 0xabcd);
        assert_eq!(word(&memory, 0x10002), 0x0ff8);
        assert_eq!(word(&memory, 0x10008), 0x1110);
        assert!(memory[..0x10000].iter().all(|&value| value == 0));
        assert!(memory[0x10010..].iter().all(|&value| value == 0));
        assert_eq!(fs::read(&path).unwrap(), source);
    }

    #[test]
    fn mz_loading_rejects_invalid_headers_and_relocations() {
        let scratch = Scratch::new();
        let data = scratch.player_data();
        let path = scratch.0.join("unpacked/1.000.exe");
        for invalid in [b"MZ".to_vec(), vec![0; 64]] {
            fs::write(&path, invalid).unwrap();
            assert!(data.memory(1).is_err());
        }
        for (offset, value, message) in [
            (4, 0, "MZ extent"),
            (2, 65, "data extent"),
            (8, 5, "data extent"),
            (0x18, 48, "relocation table"),
            (0x1c, 15, "Relocation outside module"),
        ] {
            let mut source = invented_mz();
            put_word(&mut source, offset, value);
            fs::write(&path, source).unwrap();
            assert!(data.memory(1).unwrap_err().contains(message), "{message}");
        }
        assert!(data.memory(9).unwrap_err().contains("Invalid part"));
    }

    #[test]
    fn little_endian_word_helpers_preserve_signed_boundaries() {
        let values = [0, 0, 0xff, 0x7f, 0, 0x80, 0xff, 0xff];
        assert_eq!(word(&values, 4), 32768);
        assert_eq!(signed(&values, 6), -1);
        assert_eq!(words(&values, 0, 4), [0, 32767, -32768, -1]);
    }

    #[test]
    fn mode_x_scanout_interleaves_planes_across_address_wraparound() {
        let mut video = Vga::new();
        for (plane, values) in video.planes.iter_mut().enumerate() {
            values[65535] = 10 + plane as u8;
            values[0] = 20 + plane as u8;
            values[1] = 30 + plane as u8;
        }
        video.mode_x(65535, 1, 8, 2);
        assert_eq!(
            video.indices,
            [
                10, 11, 12, 13, 20, 21, 22, 23, 20, 21, 22, 23, 30, 31, 32, 33
            ]
        );
        assert_eq!((video.width, video.height), (8, 2));
    }

    #[test]
    fn ega_scanout_combines_plane_bits_in_most_significant_bit_order() {
        let mut video = Vga::new();
        for (plane, byte) in [0x55, 0x33, 0x0f, 0].into_iter().enumerate() {
            video.planes[plane][42] = byte;
            video.planes[plane][43] = byte | if plane == 3 { 0xff } else { 0 };
        }
        video.ega(42, 1, 8, 2);
        assert_eq!(video.indices, (0..16).collect::<Vec<_>>());
    }

    #[test]
    fn packed_pixels_roundtrip_through_planes_and_six_bit_dac() {
        let mut video = Vga::new();
        let pixels = [2, 0, 1, 2, 1, 2, 0, 1];
        video.palette[..9].copy_from_slice(&[0, 16, 63, 64, 95, 127, 1, 32, 62]);
        video.packed(&pixels, 4, 2);
        assert_eq!(video.indices, pixels);
        video.indices.fill(255);
        video.mode_x(0, 1, 4, 2);
        assert_eq!(video.indices, pixels);
        assert_eq!(&video.rgb()[..9], &[4, 130, 251, 0, 65, 255, 0, 125, 255]);
    }
}
