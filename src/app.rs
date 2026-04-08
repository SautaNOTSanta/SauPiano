use std::collections::{HashMap, HashSet};
use std::sync::mpsc;

use egui::{Color32, Key, Pos2, Rect, RichText, Rounding, Sense, Stroke, Vec2};

use crate::keyboard::{build_key_map, is_black_key, note_name};
use crate::midi_out::WindowsMidiOut;
use crate::midi_player::{MidiEvent, MidiPlayer};
use crate::sf2_engine::Sf2Engine;

#[derive(PartialEq, Clone, Copy)]
pub enum BackendMode {
    WindowsMidi,
    Sf2,
}

pub struct SauPianoApp {
    key_map:      HashMap<Key, u8>,
    active_notes: HashSet<u8>,
    octave_shift: i32,
    velocity:     u8,

    mode:     BackendMode,
    midi_out: Option<WindowsMidiOut>,
    sf2:      Option<Sf2Engine>,

    midi_ports:    Vec<String>,
    selected_port: usize,

    sf2_path:     Option<String>,
    selected_preset: usize,

    midi_player:    MidiPlayer,
    midi_rx:        mpsc::Receiver<MidiEvent>,
    midi_file_path: Option<String>,
    is_playing:     bool,

    status:         String,
    show_help:      bool,
}

impl SauPianoApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_custom_fonts(&cc.egui_ctx);

        let (tx, rx) = mpsc::channel::<MidiEvent>();

        let midi_ports = WindowsMidiOut::list_ports().unwrap_or_default();
        let selected_port = midi_ports
            .iter()
            .position(|n| n.contains("Microsoft GS Wavetable"))
            .unwrap_or(0);

        let midi_out = WindowsMidiOut::connect(selected_port).ok();
        let status = if midi_out.is_some() {
            format!("已连接 / Connected: {}", midi_ports.get(selected_port).cloned().unwrap_or_default())
        } else {
            "未连接 MIDI 输出，请在设置中选择端口 / No MIDI output is connected. Please select a port in the settings.".into()
        };

        Self {
            key_map:      build_key_map(),
            active_notes: HashSet::new(),
            octave_shift: 0,
            velocity:     100,

            mode: BackendMode::WindowsMidi,
            midi_out,
            sf2: None,

            midi_ports,
            selected_port,

            sf2_path:        None,
            selected_preset: 0,

            midi_player: MidiPlayer::new(tx),
            midi_rx:     rx,
            midi_file_path: None,
            is_playing:  false,

            status,
            show_help: false,
        }
    }

    fn note_on(&mut self, note: u8, vel: u8) {
        let shifted = note as i32 + self.octave_shift * 12;
        if shifted < 0 || shifted > 127 { return; }
        let note = shifted as u8;
        if self.active_notes.contains(&note) { return; }

        self.active_notes.insert(note);
        match self.mode {
            BackendMode::WindowsMidi => {
                if let Some(out) = &mut self.midi_out {
                    out.note_on(0, note, vel);
                }
            }
            BackendMode::Sf2 => {
                if let Some(sf) = &self.sf2 {
                    sf.note_on(0, note as i32, vel as i32);
                }
            }
        }
    }

    fn note_off(&mut self, note: u8) {
        let shifted = note as i32 + self.octave_shift * 12;
        if shifted < 0 || shifted > 127 { return; }
        let note = shifted as u8;

        self.active_notes.remove(&note);
        match self.mode {
            BackendMode::WindowsMidi => {
                if let Some(out) = &mut self.midi_out {
                    out.note_off(0, note);
                }
            }
            BackendMode::Sf2 => {
                if let Some(sf) = &self.sf2 {
                    sf.note_off(0, note as i32);
                }
            }
        }
    }

    fn all_notes_off(&mut self) {
        self.active_notes.clear();
        match self.mode {
            BackendMode::WindowsMidi => {
                if let Some(out) = &mut self.midi_out {
                    out.all_notes_off();
                }
            }
            BackendMode::Sf2 => {
                if let Some(sf) = &self.sf2 {
                    sf.all_notes_off();
                }
            }
        }
    }

    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            if let egui::Event::Key { key, pressed, repeat, .. } = event {
                if repeat { continue; }
                if let Some(&base_note) = self.key_map.get(&key) {
                    if pressed {
                        self.note_on(base_note, self.velocity);
                    } else {
                        self.note_off(base_note);
                    }
                } else {
                    if pressed {
                        match key {
                            Key::ArrowUp   => self.octave_shift = (self.octave_shift + 1).min(4),
                            Key::ArrowDown => self.octave_shift = (self.octave_shift - 1).max(-4),
                            Key::Escape    => self.all_notes_off(),
                            Key::F1        => self.show_help = !self.show_help,
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    fn poll_midi_events(&mut self) {
        while let Ok(ev) = self.midi_rx.try_recv() {
            match ev {
                MidiEvent::NoteOn { channel, key, vel } => {
                    match self.mode {
                        BackendMode::WindowsMidi => {
                            if let Some(out) = &mut self.midi_out {
                                out.note_on(channel, key, vel);
                            }
                        }
                        BackendMode::Sf2 => {
                            if let Some(sf) = &self.sf2 {
                                sf.note_on(channel as i32, key as i32, vel as i32);
                            }
                        }
                    }
                    self.active_notes.insert(key);
                }
                MidiEvent::NoteOff { channel, key } => {
                    match self.mode {
                        BackendMode::WindowsMidi => {
                            if let Some(out) = &mut self.midi_out {
                                out.note_off(channel, key);
                            }
                        }
                        BackendMode::Sf2 => {
                            if let Some(sf) = &self.sf2 {
                                sf.note_off(channel as i32, key as i32);
                            }
                        }
                    }
                    self.active_notes.remove(&key);
                }
                MidiEvent::ProgramChange { channel, program } => {
                    if let Some(out) = &mut self.midi_out {
                        out.program_change(channel, program);
                    }
                }
                MidiEvent::ControlChange { channel, ctrl, value } => {
                    if let Some(out) = &mut self.midi_out {
                        out.control_change(channel, ctrl, value);
                    }
                }
                MidiEvent::PlaybackDone => {
                    self.is_playing = false;
                    self.active_notes.clear();
                    self.status = "▶ 播放完成 / Playback complete".into();
                }
            }
        }
    }

    fn draw_piano(&self, ui: &mut egui::Ui) {
        const START_NOTE: u8 = 36;
        const END_NOTE:   u8 = 96;

        let white_w = 28.0_f32;
        let white_h = 100.0_f32;
        let black_w = 18.0_f32;
        let black_h = 62.0_f32;

        let white_count = (START_NOTE..=END_NOTE)
            .filter(|&n| !is_black_key(n))
            .count();

        let total_w = white_count as f32 * white_w;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(total_w, white_h + 4.0), Sense::hover());
        let painter = ui.painter_at(rect);

        let origin = rect.left_top();

        let mut white_x: Vec<(u8, f32)> = Vec::new();
        let mut wx = 0.0f32;
        for note in START_NOTE..=END_NOTE {
            if !is_black_key(note) {
                white_x.push((note, wx));
                wx += white_w;
            }
        }

        for &(note, x) in &white_x {
            let shifted = note as i32 + self.octave_shift * 12;
            let active = shifted >= 0
                && shifted <= 127
                && self.active_notes.contains(&(shifted as u8));

            let key_rect = Rect::from_min_size(
                Pos2::new(origin.x + x + 1.0, origin.y + 2.0),
                Vec2::new(white_w - 2.0, white_h),
            );
            let fill = if active {
                Color32::from_rgb(100, 180, 255)
            } else {
                Color32::WHITE
            };
            painter.rect(key_rect, Rounding::same(3.0), fill, Stroke::new(1.0, Color32::GRAY));

            if note % 12 == 0 {
                painter.text(
                    Pos2::new(key_rect.center().x, key_rect.bottom() - 12.0),
                    egui::Align2::CENTER_CENTER,
                    note_name(note),
                    egui::FontId::proportional(9.0),
                    Color32::DARK_GRAY,
                );
            }
        }

        let white_map: HashMap<u8, f32> = white_x.iter().cloned().collect();
        for note in START_NOTE..=END_NOTE {
            if !is_black_key(note) { continue; }

            let left_white = (0..note).rev().find(|&n| !is_black_key(n));
            let x = match left_white.and_then(|n| white_map.get(&n)) {
                Some(&lx) => lx + white_w - black_w / 2.0,
                None      => continue,
            };

            let shifted = note as i32 + self.octave_shift * 12;
            let active  = shifted >= 0
                && shifted <= 127
                && self.active_notes.contains(&(shifted as u8));

            let key_rect = Rect::from_min_size(
                Pos2::new(origin.x + x, origin.y + 2.0),
                Vec2::new(black_w, black_h),
            );
            let fill = if active {
                Color32::from_rgb(60, 140, 230)
            } else {
                Color32::from_gray(30)
            };
            painter.rect(key_rect, Rounding::same(2.0), fill, Stroke::new(1.0, Color32::BLACK));
        }
    }
}

impl eframe::App for SauPianoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();

        self.handle_keyboard(ctx);

        self.poll_midi_events();

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(RichText::new("🎹 SauPiano v0.01").strong());
                ui.separator();

                ui.label("八度 / Octave:");
                if ui.button("▼").clicked() {
                    self.octave_shift = (self.octave_shift - 1).max(-4);
                    self.all_notes_off();
                }
                ui.label(format!("{:+}", self.octave_shift));
                if ui.button("▲").clicked() {
                    self.octave_shift = (self.octave_shift + 1).min(4);
                    self.all_notes_off();
                }

                ui.separator();

                ui.label("力度 / Velocity:");
                let mut vel = self.velocity as f32;
                if ui.add(egui::Slider::new(&mut vel, 1.0..=127.0).integer()).changed() {
                    self.velocity = vel as u8;
                }

                ui.separator();
                if ui.button("🔇 全止音 / Full mute").clicked() {
                    self.all_notes_off();
                }
                ui.separator();
                if ui.button("帮助 / Help (F1)").clicked() {
                    self.show_help = !self.show_help;
                }
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                if !self.active_notes.is_empty() {
                    let notes: Vec<String> = {
                        let mut v: Vec<u8> = self.active_notes.iter().cloned().collect();
                        v.sort();
                        v.iter().map(|&n| note_name(n)).collect()
                    };
                    ui.separator();
                    ui.label(format!("🎵 {}", notes.join(" ")));
                }
            });
        });

        egui::SidePanel::left("settings").min_width(260.0).show(ctx, |ui| {
            ui.heading("音频设置 / Audio Setting");
            ui.separator();

            ui.label("音源模式 / Audio Source Mode:");
            ui.horizontal(|ui| {
                if ui.selectable_label(
                    self.mode == BackendMode::WindowsMidi, "Windows MIDI"
                ).clicked() {
                    self.mode = BackendMode::WindowsMidi;
                    self.all_notes_off();
                }
                if ui.selectable_label(
                    self.mode == BackendMode::Sf2, "SF2 音源 / SF2 Inst"
                ).clicked() {
                    self.mode = BackendMode::Sf2;
                    self.all_notes_off();
                }
            });

            ui.add_space(8.0);

            match self.mode {
                BackendMode::WindowsMidi => {
                    ui.label("MIDI 输出端口 / MIDI output port:");
                    let ports = self.midi_ports.clone();
                    let mut changed = false;
                    egui::ComboBox::from_id_source("midi_port")
                        .selected_text(
                            ports.get(self.selected_port).cloned()
                                .unwrap_or_else(|| "（无端口）/ (No port)".into()),
                        )
                        .show_ui(ui, |ui| {
                            for (i, name) in ports.iter().enumerate() {
                                if ui.selectable_label(self.selected_port == i, name).clicked() {
                                    self.selected_port = i;
                                    changed = true;
                                }
                            }
                        });

                    if changed || (self.midi_out.is_none() && !ports.is_empty()) {
                        self.all_notes_off();
                        match WindowsMidiOut::connect(self.selected_port) {
                            Ok(out) => {
                                self.midi_out = Some(out);
                                self.status = format!(
                                    "已连接 / Connected: {}",
                                    ports.get(self.selected_port).cloned().unwrap_or_default()
                                );
                            }
                            Err(e) => {
                                self.status = format!("连接失败 / Connection failed: {}", e);
                            }
                        }
                    }

                    if ui.button("刷新端口").clicked() {
                        self.midi_ports = WindowsMidiOut::list_ports().unwrap_or_default();
                    }
                }

                BackendMode::Sf2 => {
                    ui.label("SF2 音源文件 / SF2 Inst:");
                    ui.horizontal(|ui| {
                        ui.label(
                            self.sf2_path.as_deref().unwrap_or("未选择 / Not selected"),
                        );
                        if ui.button("浏览").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("SF2 音源 / SF2 Inst", &["sf2"])
                                .pick_file()
                            {
                                let path_str = path.display().to_string();
                                self.all_notes_off();
                                match Sf2Engine::new(&path_str) {
                                    Ok(engine) => {
                                        self.sf2_path = Some(path_str.clone());
                                        self.selected_preset = 0;
                                        self.status = format!("已载入 SF2 / SF2 Loaded: {}", path_str);
                                        self.sf2 = Some(engine);
                                    }
                                    Err(e) => {
                                        self.status = format!("载入 SF2 失败 / Failed to load SF2: {}", e);
                                    }
                                }
                            }
                        }
                    });

                    if let Some(sf) = &self.sf2 {
                        ui.add_space(6.0);
                        ui.label("选择音色 / Inst selection:");
                        let presets = sf.presets
                            .iter()
                            .map(|p| format!("[{:03}:{:03}] {}", p.bank, p.preset, p.name))
                            .collect::<Vec<_>>();
                        let current = presets.get(self.selected_preset)
                            .cloned().unwrap_or_default();
                        let mut changed = false;
                        egui::ComboBox::from_id_source("sf2_preset")
                            .width(230.0)
                            .selected_text(current)
                            .show_ui(ui, |ui| {
                                for (i, name) in presets.iter().enumerate() {
                                    if ui.selectable_label(self.selected_preset == i, name).clicked() {
                                        self.selected_preset = i;
                                        changed = true;
                                    }
                                }
                            });
                        if changed {
                            let p = &sf.presets[self.selected_preset];
                            sf.select_preset(p.bank, p.preset);
                            self.status = format!("音色: {}", p.name);
                        }
                    }
                }
            }

            ui.add_space(12.0);
            ui.separator();

            ui.heading("MIDI 文件播放 / MIDI File Playback");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    self.midi_file_path.as_deref().unwrap_or("未选择文件 / Not selected"),
                );
                if ui.button("📂").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("MIDI 文件 / Midi Files", &["mid", "midi"])
                        .pick_file()
                    {
                        self.midi_file_path = Some(path.display().to_string());
                        self.status = format!("MIDI: {}", path.display());
                    }
                }
            });

            ui.horizontal(|ui| {
                let can_play = self.midi_file_path.is_some();

                if ui.add_enabled(
                    can_play && !self.is_playing,
                    egui::Button::new("▶ 播放 / Play"),
                ).clicked() {
                    let path = self.midi_file_path.clone().unwrap();
                    match std::fs::read(&path) {
                        Ok(bytes) => {
                            self.all_notes_off();
                            match self.midi_player.play(&bytes) {
                                Ok(_) => {
                                    self.is_playing = true;
                                    self.status = format!("▶ 播放中 / Playing: {}", path);
                                }
                                Err(e) => {
                                    self.status = format!("播放失败 / Failed to play: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            self.status = format!("读取文件失败 / Failed to read the file: {}", e);
                        }
                    }
                }

                if ui.add_enabled(
                    self.is_playing,
                    egui::Button::new("停止 / Stop"),
                ).clicked() {
                    self.midi_player.stop();
                    self.all_notes_off();
                    self.is_playing = false;
                    self.status = "已停止 / Stopped".into();
                }
            });

            ui.add_space(12.0);
            ui.separator();
            ui.heading("键位参考 // Keyboard reference");
            ui.monospace("上行 Up: Q W E R T Y U I O P");
            ui.monospace("黑键 Black Notes: 2 3   5 6 7   9 0");
            ui.monospace("下行 Down: Z X C V B N M");
            ui.monospace("黑键 Black Notes:  S D   G H J");
            ui.add_space(4.0);
            ui.label("↑↓ 方向键: 切换八度 / Arrow keys: Switch octaves");
            ui.label("Esc: 全部止音 / Mute All");
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.heading(format!(
                    "当前八度偏移 / Current octave offset: {:+}  |  基准 Base: C4=60  |  力度 Velocity: {}",
                    self.octave_shift, self.velocity
                ));
                ui.add_space(20.0);
                self.draw_piano(ui);
            });

            if self.show_help {
                egui::Window::new("帮助 / Help - SauPiano v0.01")
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label("SauPiano v0.01 使用说明 Manual");
                        ui.separator();
                        ui.label("Q行/Z行: 弹奏钢琴音符 / Q/Z: Play piano notes");
                        ui.label("数字行: 黑键（升号音）/ Row: Black keys (sharp notes)");
                        ui.label("↑↓ 方向键: 八度 +/- / ↑↓ Arrow keys: Octave +/-");
                        ui.label("Esc: 停止所有音符 / Stop all notes");
                        ui.label("F1: 显示/隐藏帮助 / Show/Hide Help");
                        ui.separator();
                        ui.label("Windows MIDI 模式走 WinMM → MS GS Wavetable Synth → gm.dls");
                        ui.label("SF2 模式用 rustysynth 合成，cpal 输出");
                        if ui.button("关闭 / Close").clicked() {
                            self.show_help = false;
                        }
                    });
            }
        });
    }
}

fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "my_chinese_font".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/msyhbd.ttc")),
    );

    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "my_chinese_font".to_owned());

    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("my_chinese_font".to_owned());

    ctx.set_fonts(fonts);
}