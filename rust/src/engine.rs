use crate::{
    data::Data,
    music::Music,
    scenes::{
        dots::Dots, ending::Ending, field::Field, greetings::Greetings, intro::Intro,
        plasma::Plasma, rings::Rings, scroller::Scroller,
    },
    vga::Vga,
};

pub const SAMPLE_RATE: u64 = 16_000;
pub const FRAME_RATE: u64 = 70;
pub const SEQUENCE: [u8; 8] = [1, 2, 6, 5, 3, 4, 7, 8];
enum Scene {
    Intro(Box<Intro>),
    Scroller(Box<Scroller>),
    Rings(Box<Rings>),
    Field(Box<Field>),
    Dots(Box<Dots>),
    Plasma(Box<Plasma>),
    Greetings(Box<Greetings>),
    Ending(Box<Ending>),
}
impl Scene {
    fn new(part: u8, data: &Data, vga: &mut Vga) -> Result<Self, String> {
        Ok(match part {
            1 => Self::Intro(Box::new(Intro::new(data, vga)?)),
            2 => Self::Scroller(Box::new(Scroller::new(data, vga)?)),
            6 => Self::Rings(Box::new(Rings::new(data, vga)?)),
            5 => Self::Field(Box::new(Field::new(data, vga)?)),
            3 => Self::Dots(Box::new(Dots::new(data, vga)?)),
            4 => Self::Plasma(Box::new(Plasma::new(data, vga)?)),
            7 => Self::Greetings(Box::new(Greetings::new(data, vga)?)),
            8 => Self::Ending(Box::new(Ending::new(data, vga)?)),
            _ => return Err("Unknown scene".into()),
        })
    }
    fn step(&mut self, vga: &mut Vga, music: u16) -> bool {
        match self {
            Self::Intro(s) => s.step(vga, music),
            Self::Scroller(s) => s.step(vga, music),
            Self::Rings(s) => s.step(vga, music),
            Self::Field(s) => s.step(vga, music),
            Self::Dots(s) => s.step(vga, music),
            Self::Plasma(s) => s.step(vga, music),
            Self::Greetings(s) => s.step(vga, music),
            Self::Ending(s) => s.step(vga, music),
        }
    }
}

/// Fixed 70 Hz timer with exact rational audio-frame accounting.
/// Rendering and audio device scheduling are deliberately outside this core.
pub struct Engine {
    data: Data,
    scene: Scene,
    scene_index: usize,
    pub video: Vga,
    pub music: Music,
    pub frames: u64,
    pub finished: bool,
    fallback: bool,
}
impl Engine {
    pub fn new(data: Data, fallback: bool) -> Result<Self, String> {
        let mut video = Vga::new();
        let music = Music::new(&data)?;
        let scene = Scene::new(1, &data, &mut video)?;
        Ok(Self {
            data,
            scene,
            scene_index: 0,
            video,
            music,
            frames: 0,
            finished: false,
            fallback,
        })
    }
    pub fn part(&self) -> u8 {
        SEQUENCE[self.scene_index]
    }
    pub fn seconds(&self) -> f64 {
        self.frames as f64 / FRAME_RATE as f64
    }
    pub fn audio_frames(&self) -> u64 {
        self.frames * SAMPLE_RATE / FRAME_RATE
    }
    pub fn step(&mut self) -> Result<Vec<i16>, String> {
        if self.finished {
            return Ok(Vec::new());
        }
        let position = if self.fallback {
            0x7fff
        } else {
            self.music.position()
        };
        let done = self.scene.step(&mut self.video, position);
        if let Scene::Intro(s) = &self.scene
            && let Some(frame) = s.cue_frame()
        {
            self.music.intro_cue(frame);
        }
        let samples = (self.frames + 1) * SAMPLE_RATE / FRAME_RATE - self.audio_frames();
        let audio = if self.fallback {
            vec![0; samples as usize * 2]
        } else {
            self.music.render(samples as usize)
        };
        self.frames += 1;
        if done {
            if self.scene_index + 1 == SEQUENCE.len() {
                self.finished = true;
            } else {
                self.scene_index += 1;
                self.scene = Scene::new(self.part(), &self.data, &mut self.video)?;
            }
        }
        // A corrupt or incompatible data set must not make an unattended export hang.
        if self.frames > FRAME_RATE * 600 {
            return Err(format!(
                "Scene {} exceeded the ten-minute playback limit",
                self.part()
            ));
        }
        Ok(audio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn complete_music_and_fallback_sequences_preserve_final_frame() {
        let data = crate::test_support::data();
        for fallback in [false, true] {
            let mut e = Engine::new(data.clone(), fallback).unwrap();
            let mut order = vec![1];
            let mut samples = 0u64;
            while !e.finished {
                let pcm = e.step().unwrap();
                samples += pcm.len() as u64 / 2;
                if order.last().copied() != Some(e.part()) {
                    order.push(e.part());
                }
            }
            assert_eq!(order, SEQUENCE);
            assert_eq!(samples, e.audio_frames());
            assert!(e.frames > 70 * 150 && e.frames < 70 * 300);
            assert_eq!((e.video.width, e.video.height), (320, 200));
            assert_eq!(e.video.indices[100 * 320 + 160], 1);
            assert_eq!(e.video.indices.iter().filter(|&&p| p != 0).count(), 1);
            if !fallback {
                assert_eq!(e.music.position() >> 8, 53);
                assert!(!e.music.ended());
            }
            assert!(e.step().unwrap().is_empty());
        }
    }
}
