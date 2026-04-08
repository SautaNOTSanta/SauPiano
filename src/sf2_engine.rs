use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, Mutex};
use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};

pub struct PresetInfo {
    pub name:   String,
    pub bank:   i32,
    pub preset: i32,
}

pub struct Sf2Engine {
    pub presets: Vec<PresetInfo>,
    synth:      Arc<Mutex<Synthesizer>>,
    _stream:    cpal::Stream, // Hold the stream to prevent it from being dropped
}

impl Sf2Engine {
    pub fn new(sf2_path: &str) -> Result<Self> {
        // ── LOAD SF2 ──
        let file = File::open(sf2_path)
            .map_err(|e| anyhow!("无法打开 SF2 文件 / Unable to open SF2 files: {}", e))?;
        let mut reader = BufReader::new(file);
        let sf = Arc::new(
            SoundFont::new(&mut reader)
                .map_err(|e| anyhow!("解析 SF2 失败 / Analysis of SF2 Failure: {:?}", e))?,
        );

        // Collect preset list
        let presets: Vec<PresetInfo> = sf
            .get_presets()
            .iter()
            .map(|p| PresetInfo {
                name:   p.get_name().to_string(),
                bank:   p.get_bank_number() as i32,
                preset: p.get_patch_number() as i32,
            })
            .collect();

        // ── Create an audio device ──
        let host   = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("找不到音频输出设备 / No audio output device found"))?;
        let config      = device.default_output_config()?;
        let sample_rate = config.sample_rate().0;
        let channels    = config.channels() as usize;

        // ── create synthesizer
        let settings = SynthesizerSettings::new(sample_rate as i32);
        let synth = Arc::new(Mutex::new(
            Synthesizer::new(&sf, &settings)
                .map_err(|e| anyhow!("创建合成器失败 / Failed to create synthesizer: {:?}", e))?,
        ));

        // ── Establish an audio stream
        let synth_cb = synth.clone();
        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _| {
                    let frames = data.len() / channels;
                    let mut left  = vec![0f32; frames];
                    let mut right = vec![0f32; frames];

                    if let Ok(mut s) = synth_cb.try_lock() {
                        s.render(&mut left, &mut right);
                    }

                    for (i, frame) in data.chunks_mut(channels).enumerate() {
                        if i < frames {
                            frame[0] = left[i];
                            if channels > 1 {
                                frame[1] = right[i];
                            }
                        }
                    }
                },
                |err| eprintln!("[SF2音频错误 / SF2 Audio Error] {}", err),
                None,
            )
            .map_err(|e| anyhow!("建立音频流失败 / Failed to establish an audio stream: {}", e))?;

        stream.play()?;

        Ok(Self { presets, synth, _stream: stream })
    }

    pub fn note_on(&self, channel: i32, key: i32, velocity: i32) {
        if let Ok(mut s) = self.synth.lock() {
            s.note_on(channel, key, velocity);
        }
    }

    pub fn note_off(&self, channel: i32, key: i32) {
        if let Ok(mut s) = self.synth.lock() {
            s.note_off(channel, key);
        }
    }

    /// Switch sounds: Bank select + Program change
    pub fn select_preset(&self, bank: i32, preset: i32) {
        if let Ok(mut s) = self.synth.lock() {

            // Bank select MSB/LSB
            s.process_midi_message(0, 0xB0, 0x00, bank >> 7);
            s.process_midi_message(0, 0xB0, 0x20, bank & 0x7F);

            // Program change
            s.process_midi_message(0, 0xC0, preset, 0);
        }
    }

    pub fn all_notes_off(&self) {
        if let Ok(mut s) = self.synth.lock() {
            for ch in 0..16 {
                s.process_midi_message(ch, 0xB0, 123, 0); // All Notes Off
                s.process_midi_message(ch, 0xB0, 120, 0); // All Sound Off
            }
        }
    }

    pub fn set_volume(&self, channel: i32, volume: i32) {
        if let Ok(mut s) = self.synth.lock() {
            s.process_midi_message(channel, 0xB0, 7, volume);
        }
    }
}