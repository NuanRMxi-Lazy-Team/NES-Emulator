use gilrs::{Axis, Button, Gilrs};

pub const BTN_A: u8 = 0x01;
pub const BTN_B: u8 = 0x02;
pub const BTN_SELECT: u8 = 0x04;
pub const BTN_START: u8 = 0x08;
pub const BTN_UP: u8 = 0x10;
pub const BTN_DOWN: u8 = 0x20;
pub const BTN_LEFT: u8 = 0x40;
pub const BTN_RIGHT: u8 = 0x80;

pub struct Input {
    gilrs: Gilrs,
    frame: u32,
    pub connected: bool,
}

impl Input {
    pub fn new() -> Option<Self> {
        Gilrs::new()
            .ok()
            .map(|gilrs| Input {
                gilrs,
                frame: 0,
                connected: false,
            })
    }

    pub fn poll(&mut self) -> u8 {
        while self.gilrs.next_event().is_some() {}

        self.frame = self.frame.wrapping_add(1);
        let turbo = (self.frame / 3) % 2 == 0;

        let mut b = 0u8;
        self.connected = false;

        if let Some((_id, gp)) = self.gilrs.gamepads().next() {
            self.connected = true;

            if gp.is_pressed(Button::South) {
                b |= BTN_A;
            }
            if gp.is_pressed(Button::East) {
                b |= BTN_B;
            }
            if gp.is_pressed(Button::West) && turbo {
                b |= BTN_A;
            }
            if gp.is_pressed(Button::North) && turbo {
                b |= BTN_B;
            }

            if gp.is_pressed(Button::DPadUp) {
                b |= BTN_UP;
            }
            if gp.is_pressed(Button::DPadDown) {
                b |= BTN_DOWN;
            }
            if gp.is_pressed(Button::DPadLeft) {
                b |= BTN_LEFT;
            }
            if gp.is_pressed(Button::DPadRight) {
                b |= BTN_RIGHT;
            }

            if gp.is_pressed(Button::Start) {
                b |= BTN_START;
            }
            if gp.is_pressed(Button::Select) {
                b |= BTN_SELECT;
            }

            const THRESHOLD: f32 = 0.5;
            if let Some(x) = gp.axis_data(Axis::LeftStickX) {
                if x.value() < -THRESHOLD {
                    b |= BTN_LEFT;
                }
                if x.value() > THRESHOLD {
                    b |= BTN_RIGHT;
                }
            }
            if let Some(y) = gp.axis_data(Axis::LeftStickY) {
                if y.value() < -THRESHOLD {
                    b |= BTN_DOWN;
                }
                if y.value() > THRESHOLD {
                    b |= BTN_UP;
                }
            }
        }

        b
    }
}
