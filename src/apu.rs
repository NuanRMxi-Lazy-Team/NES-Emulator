const DUTY: [[u8; 8]; 4] = [
    [0, 1, 0, 0, 0, 0, 0, 0],
    [0, 1, 1, 0, 0, 0, 0, 0],
    [0, 1, 1, 1, 1, 0, 0, 0],
    [1, 0, 0, 1, 1, 1, 1, 1],
];
const TRIANGLE: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11,
    12, 13, 14, 15,
];
const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30,
];
const NOISE_PERIOD: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];
const DMC_RATE: [u16; 16] = [
    428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54,
];

struct Envelope {
    start: bool,
    divider: u8,
    decay: u8,
    looped: bool,
    constant: bool,
    volume: u8,
}

impl Envelope {
    fn new() -> Self {
        Envelope {
            start: false,
            divider: 0,
            decay: 0,
            looped: false,
            constant: false,
            volume: 0,
        }
    }

    fn clock(&mut self) {
        if self.start {
            self.start = false;
            self.decay = 15;
            self.divider = self.volume;
        } else if self.divider == 0 {
            self.divider = self.volume;
            if self.decay > 0 {
                self.decay -= 1;
            } else if self.looped {
                self.decay = 15;
            }
        } else {
            self.divider -= 1;
        }
    }

    fn output(&self) -> u8 {
        if self.constant {
            self.volume
        } else {
            self.decay
        }
    }
}

struct Pulse {
    enabled: bool,
    duty: u8,
    seq: u8,
    timer: u16,
    timer_period: u16,
    length: u8,
    envelope: Envelope,
    sweep_enabled: bool,
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    sweep_reload: bool,
    sweep_counter: u8,
    channel: u8,
}

impl Pulse {
    fn new(channel: u8) -> Self {
        Pulse {
            enabled: false,
            duty: 0,
            seq: 0,
            timer: 0,
            timer_period: 0,
            length: 0,
            envelope: Envelope::new(),
            sweep_enabled: false,
            sweep_period: 0,
            sweep_negate: false,
            sweep_shift: 0,
            sweep_reload: false,
            sweep_counter: 0,
            channel,
        }
    }

    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period * 2 + 1;
            self.seq = (self.seq + 1) & 7;
        } else {
            self.timer -= 1;
        }
    }

    fn clock_length(&mut self) {
        if self.length > 0 && !self.envelope.looped {
            self.length -= 1;
        }
    }

    fn sweep_target(&self) -> u16 {
        let delta = self.timer_period >> self.sweep_shift;
        if self.sweep_negate {
            self.timer_period
                .wrapping_sub(delta)
                .wrapping_sub(if self.channel == 1 { 1 } else { 0 })
        } else {
            self.timer_period.wrapping_add(delta)
        }
    }

    fn sweep_mute(&self) -> bool {
        self.timer_period < 8 || self.sweep_target() > 0x7FF
    }

    fn clock_sweep(&mut self) {
        if self.sweep_counter == 0 && self.sweep_enabled && self.sweep_shift > 0 && !self.sweep_mute()
        {
            self.timer_period = self.sweep_target() & 0x7FF;
        }
        if self.sweep_counter == 0 || self.sweep_reload {
            self.sweep_counter = self.sweep_period;
            self.sweep_reload = false;
        } else {
            self.sweep_counter -= 1;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled
            || self.length == 0
            || DUTY[self.duty as usize][self.seq as usize] == 0
            || self.sweep_mute()
        {
            0
        } else {
            self.envelope.output()
        }
    }

    fn write_ctrl(&mut self, val: u8) {
        self.duty = (val >> 6) & 3;
        self.envelope.looped = val & 0x20 != 0;
        self.envelope.constant = val & 0x10 != 0;
        self.envelope.volume = val & 0x0F;
    }

    fn write_sweep(&mut self, val: u8) {
        self.sweep_enabled = val & 0x80 != 0;
        self.sweep_period = (val >> 4) & 7;
        self.sweep_negate = val & 0x08 != 0;
        self.sweep_shift = val & 7;
        self.sweep_reload = true;
    }
}

struct Triangle {
    enabled: bool,
    timer: u16,
    timer_period: u16,
    seq: u8,
    length: u8,
    control: bool,
    linear_counter: u8,
    linear_reload: u8,
    linear_reload_flag: bool,
}

impl Triangle {
    fn new() -> Self {
        Triangle {
            enabled: false,
            timer: 0,
            timer_period: 0,
            seq: 0,
            length: 0,
            control: false,
            linear_counter: 0,
            linear_reload: 0,
            linear_reload_flag: false,
        }
    }

    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period;
            self.seq = (self.seq + 1) & 31;
        } else {
            self.timer -= 1;
        }
    }

    fn clock_linear(&mut self) {
        if self.linear_reload_flag {
            self.linear_counter = self.linear_reload;
        } else if self.linear_counter > 0 {
            self.linear_counter -= 1;
        }
        if !self.control {
            self.linear_reload_flag = false;
        }
    }

    fn clock_length(&mut self) {
        if self.length > 0 && !self.control {
            self.length -= 1;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled || self.length == 0 || self.linear_counter == 0 {
            0
        } else {
            TRIANGLE[self.seq as usize]
        }
    }
}

struct Noise {
    enabled: bool,
    timer: u16,
    timer_period: u16,
    mode: bool,
    shift: u16,
    length: u8,
    envelope: Envelope,
}

impl Noise {
    fn new() -> Self {
        Noise {
            enabled: false,
            timer: 0,
            timer_period: NOISE_PERIOD[0],
            mode: false,
            shift: 1,
            length: 0,
            envelope: Envelope::new(),
        }
    }

    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period;
            let bit = if self.mode {
                (self.shift ^ (self.shift >> 6)) & 1
            } else {
                (self.shift ^ (self.shift >> 1)) & 1
            };
            self.shift = (self.shift >> 1) | (bit << 14);
        } else {
            self.timer -= 1;
        }
    }

    fn clock_length(&mut self) {
        if self.length > 0 && !self.envelope.looped {
            self.length -= 1;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled || self.length == 0 || self.shift & 1 != 0 {
            0
        } else {
            self.envelope.output()
        }
    }
}

struct Dmc {
    enabled: bool,
    irq_enable: bool,
    looped: bool,
    timer: u16,
    timer_period: u16,
    output_level: u8,
    sample_addr: u8,
    sample_len: u8,
    current_addr: u16,
    bytes_remaining: u16,
    sample_buffer: Option<u8>,
    shift: u8,
    bits_remaining: u8,
    silence: bool,
    dma_pending: bool,
    dma_addr: u16,
}

impl Dmc {
    fn new() -> Self {
        Dmc {
            enabled: false,
            irq_enable: false,
            looped: false,
            timer: 0,
            timer_period: DMC_RATE[0],
            output_level: 0,
            sample_addr: 0,
            sample_len: 0,
            current_addr: 0xC000,
            bytes_remaining: 0,
            sample_buffer: None,
            shift: 0,
            bits_remaining: 8,
            silence: true,
            dma_pending: false,
            dma_addr: 0,
        }
    }

    fn restart(&mut self) {
        self.current_addr = 0xC000 | ((self.sample_addr as u16) << 6);
        self.bytes_remaining = ((self.sample_len as u16) << 4) | 1;
    }

    fn request_dma(&mut self) {
        if self.sample_buffer.is_none() && self.bytes_remaining > 0 && !self.dma_pending {
            self.dma_pending = true;
            self.dma_addr = self.current_addr;
        }
    }

    fn load(&mut self, val: u8) {
        self.sample_buffer = Some(val);
        self.dma_pending = false;
        self.current_addr = self.current_addr.wrapping_add(1);
        if self.current_addr == 0 {
            self.current_addr = 0x8000;
        }
        self.bytes_remaining = self.bytes_remaining.saturating_sub(1);
    }

    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_period;
            if self.bits_remaining == 0 {
                self.bits_remaining = 8;
                if let Some(b) = self.sample_buffer.take() {
                    self.shift = b;
                    self.silence = false;
                    self.request_dma();
                } else {
                    self.silence = true;
                }
            }
            if !self.silence {
                if self.shift & 1 != 0 {
                    if self.output_level <= 125 {
                        self.output_level += 2;
                    }
                } else if self.output_level >= 2 {
                    self.output_level -= 2;
                }
            }
            self.shift >>= 1;
            self.bits_remaining = self.bits_remaining.saturating_sub(1);
            if self.bytes_remaining == 0 && self.sample_buffer.is_none() && self.looped && self.enabled
            {
                self.restart();
                self.request_dma();
            }
        } else {
            self.timer -= 1;
        }
    }

    fn output(&self) -> u8 {
        self.output_level
    }
}

pub struct Apu {
    pulse1: Pulse,
    pulse2: Pulse,
    triangle: Triangle,
    noise: Noise,
    dmc: Dmc,
    frame_cycle: u32,
    frame_step: u8,
    frame_mode: u8,
    frame_irq_inhibit: bool,
    pub frame_irq: bool,
    sample_counter: f64,
    sample_period: f64,
    sample_sum: f64,
    sample_n: u32,
    pub output: Vec<f32>,
}

impl Apu {
    pub fn new(sample_rate: f64) -> Self {
        Apu {
            pulse1: Pulse::new(1),
            pulse2: Pulse::new(2),
            triangle: Triangle::new(),
            noise: Noise::new(),
            dmc: Dmc::new(),
            frame_cycle: 0,
            frame_step: 0,
            frame_mode: 0,
            frame_irq_inhibit: false,
            frame_irq: false,
            sample_counter: 0.0,
            sample_period: 1789773.0 / sample_rate,
            sample_sum: 0.0,
            sample_n: 0,
            output: Vec::with_capacity(4096),
        }
    }

    fn clock_quarter(&mut self) {
        self.pulse1.envelope.clock();
        self.pulse2.envelope.clock();
        self.triangle.clock_linear();
        self.noise.envelope.clock();
    }

    fn clock_half(&mut self) {
        self.pulse1.clock_length();
        self.pulse2.clock_length();
        self.triangle.clock_length();
        self.noise.clock_length();
        self.pulse1.clock_sweep();
        self.pulse2.clock_sweep();
    }

    fn clock_frame(&mut self) {
        let cur = self.frame_step;
        self.frame_step += 1;
        if cur == 0 || cur == 2 || cur == 4 {
            self.clock_quarter();
        }
        if cur == 1 || cur == 3 {
            self.clock_half();
        }
        let max = if self.frame_mode == 0 { 4 } else { 5 };
        if self.frame_step >= max {
            self.frame_step = 0;
            if self.frame_mode == 0 && !self.frame_irq_inhibit {
                self.frame_irq = true;
            }
        }
    }

    fn mix(&self) -> f32 {
        let p1 = self.pulse1.output() as f32;
        let p2 = self.pulse2.output() as f32;
        let tri = self.triangle.output() as f32;
        let noise = self.noise.output() as f32;
        let dmc = self.dmc.output() as f32;

        let pulse_sum = p1 + p2;
        let pulse_out = if pulse_sum > 0.0 {
            95.88 / (8128.0 / pulse_sum + 100.0)
        } else {
            0.0
        };
        let tnd = tri / 8227.0 + noise / 12241.0 + dmc / 22638.0;
        let tnd_out = if tnd > 0.0 {
            159.79 / (1.0 / tnd + 100.0)
        } else {
            0.0
        };
        ((pulse_out + tnd_out) - 0.5).clamp(-1.0, 1.0)
    }

    pub fn clock(&mut self) {
        self.pulse1.clock_timer();
        self.pulse2.clock_timer();
        self.triangle.clock_timer();
        self.noise.clock_timer();
        self.dmc.clock_timer();

        self.frame_cycle += 1;
        if self.frame_cycle >= 7457 {
            self.frame_cycle -= 7457;
            self.clock_frame();
        }

        self.sample_sum += self.mix() as f64;
        self.sample_n += 1;
        self.sample_counter += 1.0;
        if self.sample_counter >= self.sample_period {
            self.sample_counter -= self.sample_period;
            let avg = self.sample_sum / self.sample_n.max(1) as f64;
            self.output.push(avg as f32);
            self.sample_sum = 0.0;
            self.sample_n = 0;
        }
    }

    pub fn dmc_dma_request(&self) -> Option<u16> {
        if self.dmc.dma_pending {
            Some(self.dmc.dma_addr)
        } else {
            None
        }
    }

    pub fn dmc_load(&mut self, val: u8) {
        self.dmc.load(val);
    }

    pub fn read_status(&mut self) -> u8 {
        let mut result = 0u8;
        if self.pulse1.length > 0 {
            result |= 0x01;
        }
        if self.pulse2.length > 0 {
            result |= 0x02;
        }
        if self.triangle.length > 0 {
            result |= 0x04;
        }
        if self.noise.length > 0 {
            result |= 0x08;
        }
        if self.dmc.bytes_remaining > 0 {
            result |= 0x10;
        }
        if self.frame_irq {
            result |= 0x40;
        }
        self.frame_irq = false;
        result
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x4000 => self.pulse1.write_ctrl(val),
            0x4001 => self.pulse1.write_sweep(val),
            0x4002 => self.pulse1.timer_period = (self.pulse1.timer_period & 0x700) | val as u16,
            0x4003 => {
                self.pulse1.timer_period =
                    (self.pulse1.timer_period & 0xFF) | (((val as u16) & 7) << 8);
                self.pulse1.length = LENGTH_TABLE[(val >> 3) as usize];
                self.pulse1.seq = 0;
                self.pulse1.envelope.start = true;
            }
            0x4004 => self.pulse2.write_ctrl(val),
            0x4005 => self.pulse2.write_sweep(val),
            0x4006 => self.pulse2.timer_period = (self.pulse2.timer_period & 0x700) | val as u16,
            0x4007 => {
                self.pulse2.timer_period =
                    (self.pulse2.timer_period & 0xFF) | (((val as u16) & 7) << 8);
                self.pulse2.length = LENGTH_TABLE[(val >> 3) as usize];
                self.pulse2.seq = 0;
                self.pulse2.envelope.start = true;
            }
            0x4008 => {
                self.triangle.control = val & 0x80 != 0;
                self.triangle.linear_reload = val & 0x7F;
            }
            0x400A => self.triangle.timer_period = (self.triangle.timer_period & 0x700) | val as u16,
            0x400B => {
                self.triangle.timer_period =
                    (self.triangle.timer_period & 0xFF) | (((val as u16) & 7) << 8);
                self.triangle.length = LENGTH_TABLE[(val >> 3) as usize];
                self.triangle.linear_reload_flag = true;
            }
            0x400C => {
                self.noise.envelope.looped = val & 0x20 != 0;
                self.noise.envelope.constant = val & 0x10 != 0;
                self.noise.envelope.volume = val & 0x0F;
            }
            0x400E => {
                self.noise.mode = val & 0x80 != 0;
                self.noise.timer_period = NOISE_PERIOD[(val & 0x0F) as usize];
            }
            0x400F => {
                self.noise.length = LENGTH_TABLE[(val >> 3) as usize];
                self.noise.envelope.start = true;
            }
            0x4010 => {
                self.dmc.irq_enable = val & 0x80 != 0;
                self.dmc.looped = val & 0x40 != 0;
                self.dmc.timer_period = DMC_RATE[(val & 0x0F) as usize];
            }
            0x4011 => self.dmc.output_level = val & 0x7F,
            0x4012 => self.dmc.sample_addr = val,
            0x4013 => self.dmc.sample_len = val,
            0x4015 => {
                self.pulse1.enabled = val & 0x01 != 0;
                if !self.pulse1.enabled {
                    self.pulse1.length = 0;
                }
                self.pulse2.enabled = val & 0x02 != 0;
                if !self.pulse2.enabled {
                    self.pulse2.length = 0;
                }
                self.triangle.enabled = val & 0x04 != 0;
                if !self.triangle.enabled {
                    self.triangle.length = 0;
                }
                self.noise.enabled = val & 0x08 != 0;
                if !self.noise.enabled {
                    self.noise.length = 0;
                }
                self.dmc.enabled = val & 0x10 != 0;
                if self.dmc.enabled {
                    if self.dmc.bytes_remaining == 0 {
                        self.dmc.restart();
                    }
                    self.dmc.request_dma();
                } else {
                    self.dmc.bytes_remaining = 0;
                }
            }
            0x4017 => {
                self.frame_mode = (val >> 7) & 1;
                self.frame_irq_inhibit = val & 0x40 != 0;
                if self.frame_irq_inhibit {
                    self.frame_irq = false;
                }
                self.frame_cycle = 0;
                self.frame_step = 0;
                if self.frame_mode == 1 {
                    self.clock_quarter();
                    self.clock_half();
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulse_period_matches_formula() {
        let mut apu = Apu::new(44100.0);
        apu.write(0x4015, 0x01);
        apu.write(0x4000, 0xBF);
        apu.write(0x4002, 253);
        apu.write(0x4003, 0x08);
        let mut advances = 0u32;
        let mut last = apu.pulse1.seq;
        for _ in 0..4064 {
            apu.clock();
            if apu.pulse1.seq != last {
                advances += 1;
                last = apu.pulse1.seq;
            }
        }
        assert_eq!(advances, 8, "pulse t=253 should complete one 8-step cycle per 4064 cycles");
    }

    #[test]
    fn triangle_period_matches_formula() {
        let mut apu = Apu::new(44100.0);
        apu.write(0x4015, 0x04);
        apu.write(0x4008, 0x7F);
        apu.write(0x400A, 126);
        apu.write(0x400B, 0x08);
        let mut advances = 0u32;
        let mut last = apu.triangle.seq;
        for _ in 0..4064 {
            apu.clock();
            if apu.triangle.seq != last {
                advances += 1;
                last = apu.triangle.seq;
            }
        }
        assert_eq!(advances, 32, "triangle t=126 should complete one 32-step cycle per 4064 cycles");
    }
}
