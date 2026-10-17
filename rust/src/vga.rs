/// Only the logical video memory and scanout operations used by Panic.
/// DAC components remain six-bit until desktop presentation.
#[derive(Clone)]
pub struct Vga {
    pub planes: [Vec<u8>; 4],
    pub palette: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub indices: Vec<u8>,
}
impl Default for Vga {
    fn default() -> Self {
        Self::new()
    }
}
impl Vga {
    pub fn new() -> Self {
        Self {
            planes: std::array::from_fn(|_| vec![0; 65536]),
            palette: vec![0; 768],
            width: 320,
            height: 200,
            indices: vec![0; 64000],
        }
    }
    fn size(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.indices.resize(width * height, 0);
    }
    pub fn mode_x(&mut self, start: usize, stride: usize, width: usize, height: usize) {
        self.size(width, height);
        for y in 0..height {
            for x in 0..width {
                self.indices[y * width + x] =
                    self.planes[x & 3][(start + y * stride + (x >> 2)) & 65535];
            }
        }
    }
    pub fn packed(&mut self, pixels: &[u8], width: usize, height: usize) {
        self.size(width, height);
        self.indices.copy_from_slice(&pixels[..width * height]);
        for (i, &v) in pixels[..width * height].iter().enumerate() {
            self.planes[i & 3][(i >> 2) & 65535] = v;
        }
    }
    pub fn ega(&mut self, start: usize, stride: usize, width: usize, height: usize) {
        self.size(width, height);
        for y in 0..height {
            for x in 0..width {
                let at = (start + y * stride + (x >> 3)) & 65535;
                let bit = 7 - (x & 7);
                self.indices[y * width + x] = (0..4)
                    .map(|p| ((self.planes[p][at] >> bit) & 1) << p)
                    .fold(0, |a, b| a | b);
            }
        }
    }
    pub fn rgb(&self) -> Vec<u8> {
        self.indices
            .iter()
            .flat_map(|&index| {
                self.palette[index as usize * 3..index as usize * 3 + 3]
                    .iter()
                    .map(|&v| {
                        let v = v & 63;
                        (v << 2) | (v >> 4)
                    })
            })
            .collect()
    }
}
