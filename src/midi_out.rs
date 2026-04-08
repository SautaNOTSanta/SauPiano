use anyhow::{anyhow, Result};
use midir::{MidiOutput, MidiOutputConnection};

pub struct WindowsMidiOut {
    conn: MidiOutputConnection,
}

impl WindowsMidiOut {
    /// List all available MIDI output port names
    pub fn list_ports() -> Result<Vec<String>> {
        let out = MidiOutput::new("SauPiano-lister")?;
        let names = out
            .ports()
            .iter()
            .filter_map(|p| out.port_name(p).ok())
            .collect();
        Ok(names)
    }

    /// 连接到指定端口（index 对应 list_ports() 的索引），懒得想英文注释了反正老外也不看
    pub fn connect(port_index: usize) -> Result<Self> {
        let out = MidiOutput::new("SauPiano")?;
        let ports = out.ports();
        let port = ports
            .get(port_index)
            .ok_or_else(|| anyhow!("MIDI 端口索引无效 / Invalid MIDI port index: {}", port_index))?;
        let conn = out
            .connect(port, "SauPiano-out")
            .map_err(|e| anyhow!("连接 MIDI 端口失败 / Failed to connect to the MIDI port: {}", e))?;
        Ok(Self { conn })
    }

    pub fn note_on(&mut self, channel: u8, note: u8, velocity: u8) {
        let _ = self.conn.send(&[0x90 | (channel & 0x0F), note & 0x7F, velocity & 0x7F]);
    }

    pub fn note_off(&mut self, channel: u8, note: u8) {
        let _ = self.conn.send(&[0x80 | (channel & 0x0F), note & 0x7F, 0]);
    }

    pub fn program_change(&mut self, channel: u8, program: u8) {
        let _ = self.conn.send(&[0xC0 | (channel & 0x0F), program & 0x7F]);
    }

    pub fn control_change(&mut self, channel: u8, ctrl: u8, value: u8) {
        let _ = self.conn.send(&[0xB0 | (channel & 0x0F), ctrl & 0x7F, value & 0x7F]);
    }

    pub fn all_notes_off(&mut self) {
        for ch in 0u8..16 {
            self.control_change(ch, 123, 0); // All Notes Off
            self.control_change(ch, 120, 0); // All Sound Off
        }
    }
}