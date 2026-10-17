use std::{
    fs,
    path::{Path, PathBuf},
};

/// Directory containing `files/` and `unpacked/`, or the research project root.
#[derive(Clone, Debug)]
pub struct Data {
    root: PathBuf,
    project: bool,
}

impl Data {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, String> {
        let root = root.as_ref().to_path_buf();
        let project = root.join("artifacts/derived/assets/files/P.P").is_file();
        let data = Self { root, project };
        if !data.file_path("P.P").is_file() {
            return Err(format!(
                "Panic data not found at {}. Pass --data <project-or-data-directory>.",
                data.root.display()
            ));
        }
        Ok(data)
    }
    fn file_path(&self, name: &str) -> PathBuf {
        self.root
            .join(if self.project {
                "artifacts/derived/assets/files"
            } else {
                "files"
            })
            .join(name)
    }
    pub fn file(&self, name: &str) -> Result<Vec<u8>, String> {
        let path = self.file_path(name);
        fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))
    }
    /// Original load-module layout, including relocations and zero-filled BSS.
    /// Preserves addresses used in the evidence; this is data loading, not a CPU.
    pub fn memory(&self, part: u8) -> Result<Vec<u8>, String> {
        let name = match part {
            0 => "PANIC.loader.exe".to_owned(),
            5 | 6 => format!("{part}.final.000.exe"),
            1..=8 => format!("{part}.000.exe"),
            _ => return Err(format!("Invalid part {part}")),
        };
        let path = self
            .root
            .join(if self.project {
                "artifacts/derived/unpacked"
            } else {
                "unpacked"
            })
            .join(name);
        let blob = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if blob.len() < 28 || &blob[..2] != b"MZ" {
            return Err(format!("Invalid MZ data: {}", path.display()));
        }
        let word = |o| u16::from_le_bytes([blob[o], blob[o + 1]]) as usize;
        let pages = word(4);
        if pages == 0 {
            return Err("Invalid MZ extent".into());
        }
        let end = (pages - 1) * 512 + if word(2) == 0 { 512 } else { word(2) };
        let start = word(8) * 16;
        if start > end || end > blob.len() || end - start > 0x100000 {
            return Err("Invalid MZ data extent".into());
        }
        let mut mem = vec![0; 0x110000];
        mem[0x10000..0x10000 + end - start].copy_from_slice(&blob[start..end]);
        for i in 0..word(6) {
            let at = word(0x18) + i * 4;
            if at + 4 > start {
                return Err("Invalid MZ relocation table".into());
            }
            let dest = 0x10000 + word(at + 2) * 16 + word(at);
            if dest + 2 > 0x10000 + end - start {
                return Err("Relocation outside module".into());
            }
            let value = u16::from_le_bytes([mem[dest], mem[dest + 1]]).wrapping_add(0x1000);
            mem[dest..dest + 2].copy_from_slice(&value.to_le_bytes());
        }
        Ok(mem)
    }
}

pub fn word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}
pub fn signed(bytes: &[u8], offset: usize) -> i32 {
    word(bytes, offset) as i16 as i32
}
pub fn words(bytes: &[u8], offset: usize, count: usize) -> Vec<i32> {
    (0..count).map(|i| signed(bytes, offset + i * 2)).collect()
}
