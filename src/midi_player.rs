use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

// ── play event type

#[derive(Debug, Clone)]
pub enum MidiEvent {
    NoteOn        { channel: u8, key: u8, vel: u8 },
    NoteOff       { channel: u8, key: u8 },
    ProgramChange { channel: u8, program: u8 },
    ControlChange { channel: u8, ctrl: u8, value: u8 },
    PlaybackDone,
}

// ── Structure for internal merging

enum RawEvt {
    NoteOn        { ch: u8, key: u8, vel: u8 },
    NoteOff       { ch: u8, key: u8 },
    ProgramChange { ch: u8, program: u8 },
    ControlChange { ch: u8, ctrl: u8, value: u8 },
    Tempo(u32),
}

struct Timed { us: u64, evt: RawEvt }

// ── MidiPlayer

pub struct MidiPlayer {
    stop_flag: Arc<AtomicBool>,
    sender:    std::sync::mpsc::Sender<MidiEvent>,
}

impl MidiPlayer {
    pub fn new(sender: std::sync::mpsc::Sender<MidiEvent>) -> Self {
        Self {
            stop_flag: Arc::new(AtomicBool::new(true)),
            sender,
        }
    }

    pub fn play(&mut self, bytes: &[u8]) -> Result<()> {
        // Stop playback from where it left off
        self.stop_flag.store(true, Ordering::SeqCst);

        let timed = parse_to_timed(bytes)?;
        let stop  = Arc::new(AtomicBool::new(false));
        self.stop_flag = stop.clone();

        let tx = self.sender.clone();
        thread::spawn(move || {
            playback_thread(timed, stop, tx);
        });

        Ok(())
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    pub fn is_playing(&self) -> bool {
        !self.stop_flag.load(Ordering::Relaxed)
    }
}

// ── Parse MIDI into a list of absolute-time events

fn parse_to_timed(bytes: &[u8]) -> Result<Vec<Timed>> {
    let smf = Smf::parse(bytes)?;

    let tpb = match smf.header.timing {
        Timing::Metrical(t) => t.as_int() as u64,
        _ => return Err(anyhow!("不支持 Timecode 格式的 MIDI 文件")),
    };

    // Collect all track events with absolute ticks
    let mut raw: Vec<(u64, u32, RawEvt)> = Vec::new();
    for (ti, track) in smf.tracks.iter().enumerate() {
        let mut abs: u64 = 0;
        for ev in track {
            abs += ev.delta.as_int() as u64;
            let re = match &ev.kind {
                TrackEventKind::Midi { channel, message } => {
                    let ch = channel.as_int();
                    match message {
                        MidiMessage::NoteOn { key, vel } => {
                            let v = vel.as_int();
                            if v == 0 {
                                RawEvt::NoteOff { ch, key: key.as_int() }
                            } else {
                                RawEvt::NoteOn { ch, key: key.as_int(), vel: v }
                            }
                        }
                        MidiMessage::NoteOff { key, .. } =>
                            RawEvt::NoteOff { ch, key: key.as_int() },
                        MidiMessage::ProgramChange { program } =>
                            RawEvt::ProgramChange { ch, program: program.as_int() },
                        MidiMessage::Controller { controller, value } =>
                            RawEvt::ControlChange {
                                ch,
                                ctrl:  controller.as_int(),
                                value: value.as_int(),
                            },
                        _ => continue,
                    }
                }
                TrackEventKind::Meta(MetaMessage::Tempo(t)) =>
                    RawEvt::Tempo(t.as_int()),
                _ => continue,
            };
            raw.push((abs, ti as u32, re));
        }
    }

    // Sort by absolute tick stability
    raw.sort_by_key(|(tick, ti, _)| (*tick, *ti));

    // Convert to a ms timeline (to track tempo changes)
    let mut result: Vec<Timed> = Vec::new();
    let mut tempo: u64 = 500_000; // 120 BPM default
    let mut last_tick: u64 = 0;
    let mut time_us:   u64 = 0;

    for (abs_tick, _, re) in raw {
        let dt = abs_tick - last_tick;
        time_us   += dt * tempo / tpb;
        last_tick  = abs_tick;

        match re {
            RawEvt::Tempo(t) => { tempo = t as u64; }
            other => result.push(Timed { us: time_us, evt: other }),
        }
    }

    Ok(result)
}

// ── Playback Thread (Precise Timing)

fn playback_thread(
    events: Vec<Timed>,
    stop:   Arc<AtomicBool>,
    tx:     std::sync::mpsc::Sender<MidiEvent>,
) {
    let start = Instant::now();

    for Timed { us, evt } in &events {
        if stop.load(Ordering::Relaxed) { return; }

        let target = Duration::from_micros(*us);
        let now    = start.elapsed();

        if now < target {
            let remaining = target - now;
            // 粗略 sleep，保留最后2ms自旋
            if remaining > Duration::from_millis(3) {
                thread::sleep(remaining - Duration::from_millis(2));
            }
            while start.elapsed() < target {
                if stop.load(Ordering::Relaxed) { return; }
                std::hint::spin_loop();
            }
        }

        let midi_ev = match evt {
            RawEvt::NoteOn  { ch, key, vel } =>
                MidiEvent::NoteOn  { channel: *ch, key: *key, vel: *vel },
            RawEvt::NoteOff { ch, key } =>
                MidiEvent::NoteOff { channel: *ch, key: *key },
            RawEvt::ProgramChange { ch, program } =>
                MidiEvent::ProgramChange { channel: *ch, program: *program },
            RawEvt::ControlChange { ch, ctrl, value } =>
                MidiEvent::ControlChange { channel: *ch, ctrl: *ctrl, value: *value },
            RawEvt::Tempo(_) => continue,
        };

        if tx.send(midi_ev).is_err() { return; }
    }

    let _ = tx.send(MidiEvent::PlaybackDone);
    stop.store(true, Ordering::SeqCst);
}