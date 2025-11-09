#[derive(Debug)]
pub enum Command {
    Input(Vec<InputSeq>),
    Mouse(MouseEvent),
    MouseClick(MouseEvent), // Convenience: sends press then release
    Snapshot,
    Resize(usize, usize),
}

#[derive(Debug, PartialEq)]
pub enum InputSeq {
    Standard(String),
    Cursor(String, String),
}

pub fn seqs_to_bytes(seqs: &[InputSeq], app_mode: bool) -> Vec<u8> {
    let mut bytes = Vec::new();

    for seq in seqs {
        bytes.extend_from_slice(seq_as_bytes(seq, app_mode));
    }

    bytes
}

fn seq_as_bytes(seq: &InputSeq, app_mode: bool) -> &[u8] {
    match (seq, app_mode) {
        (InputSeq::Standard(seq), _) => seq.as_bytes(),
        (InputSeq::Cursor(seq1, _seq2), false) => seq1.as_bytes(),
        (InputSeq::Cursor(_seq1, seq2), true) => seq2.as_bytes(),
    }
}

#[derive(Debug, Clone)]
pub struct MouseEvent {
    pub event_type: MouseEventType,
    pub button: MouseButton,
    pub row: usize,
    pub col: usize,
    pub modifiers: MouseModifiers,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MouseEventType {
    Press,
    Release,
    Drag,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MouseModifiers {
    pub shift: bool,
    pub alt: bool,
    pub control: bool,
}

pub fn mouse_to_bytes(event: &MouseEvent) -> Vec<u8> {
    // Base button encoding per SGR protocol
    let mut btn = match event.button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        MouseButton::WheelUp => 64,
        MouseButton::WheelDown => 65,
    };

    // Add modifier bits
    if event.modifiers.shift {
        btn += 4;
    }
    if event.modifiers.alt {
        btn += 8;
    }
    if event.modifiers.control {
        btn += 16;
    }

    // Add motion bit for drag events
    if matches!(event.event_type, MouseEventType::Drag) {
        btn += 32;
    }

    // SGR format: ESC[<btn;col;rowM (press/drag) or m (release)
    let suffix = match event.event_type {
        MouseEventType::Press | MouseEventType::Drag => 'M',
        MouseEventType::Release => 'm',
    };

    format!("\x1b[<{};{};{}{}", btn, event.col, event.row, suffix).into_bytes()
}
