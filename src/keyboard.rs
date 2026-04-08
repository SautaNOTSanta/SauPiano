use std::collections::HashMap;
use egui::Key;

/// Build default key mapping table
/// Layout: Two-row piano style
///
/// Bottom row (Z row): C3–B3 (white keys + interspersed black keys)
/// Top row (Q row): C4–E5 (white keys + number row as black keys)
///
///  Number row: [ 2 ] [ 3 ]   [ 5 ] [ 6 ] [ 7 ]   [ 9 ] [ 0 ]
///  Q row:   [Q] [W] [E] [R] [T] [Y] [U] [I] [O] [P]
///               C4  D4  E4  F4  G4  A4  B4  C5  D5  E5
///
///  Letter row: [S] [D]   [G] [H] [J]
///  Z row:   [Z] [X] [C] [V] [B] [N] [M]
///               C3  D3  E3  F3  G3  A3  B3
pub fn build_key_map() -> HashMap<Key, u8> {
    let mut map = HashMap::new();

    // ── Downward (Z row) — Starting at C3 —
    map.insert(Key::Z, 48); // C3
    map.insert(Key::S, 49); // C#3
    map.insert(Key::X, 50); // D3
    map.insert(Key::D, 51); // D#3
    map.insert(Key::C, 52); // E3
    map.insert(Key::V, 53); // F3
    map.insert(Key::G, 54); // F#3
    map.insert(Key::B, 55); // G3
    map.insert(Key::H, 56); // G#3
    map.insert(Key::N, 57); // A3
    map.insert(Key::J, 58); // A#3
    map.insert(Key::M, 59); // B3

    // ── Upward (Q row) — Starting at C4 —
    map.insert(Key::Q, 60);    // C4
    map.insert(Key::Num2, 61); // C#4
    map.insert(Key::W, 62);    // D4
    map.insert(Key::Num3, 63); // D#4
    map.insert(Key::E, 64);    // E4
    map.insert(Key::R, 65);    // F4
    map.insert(Key::Num5, 66); // F#4
    map.insert(Key::T, 67);    // G4
    map.insert(Key::Num6, 68); // G#4
    map.insert(Key::Y, 69);    // A4
    map.insert(Key::Num7, 70); // A#4
    map.insert(Key::U, 71);    // B4
    map.insert(Key::I, 72);    // C5
    map.insert(Key::Num9, 73); // C#5
    map.insert(Key::O, 74);    // D5
    map.insert(Key::Num0, 75); // D#5
    map.insert(Key::P, 76);    // E5

    map
}

/// Convert MIDI note numbers to readable names, such as 60 → “C4”
pub fn note_name(note: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F",
        "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = (note as i32 / 12) - 1;
    format!("{}{}", NAMES[(note % 12) as usize], octave)
}

/// is it black note???
pub fn is_black_key(note: u8) -> bool {
    matches!(note % 12, 1 | 3 | 6 | 8 | 10)
}