pub struct Controller {
    pub buttons: u8,
    shift: u8,
    strobe: bool,
}

impl Controller {
    pub fn new() -> Self {
        Controller {
            buttons: 0,
            shift: 0,
            strobe: false,
        }
    }

    pub fn write(&mut self, val: u8) {
        self.strobe = val & 1 != 0;
        if self.strobe {
            self.shift = self.buttons;
        }
    }

    pub fn read(&mut self) -> u8 {
        let v = self.shift & 1;
        if self.strobe {
            self.shift = self.buttons;
        } else {
            self.shift = (self.shift >> 1) | 0x80;
        }
        v
    }
}
