use crate::bus::Bus;

pub const C: u8 = 0x01;
pub const Z: u8 = 0x02;
pub const I: u8 = 0x04;
pub const D: u8 = 0x08;
pub const B: u8 = 0x10;
pub const U: u8 = 0x20;
pub const V: u8 = 0x40;
pub const N: u8 = 0x80;

#[rustfmt::skip]
const CYCLES: [u8; 256] = [
    7,6,0,8,3,3,5,5,3,2,2,2,4,4,6,6,
    2,5,0,8,4,4,6,6,2,4,2,7,4,4,7,7,
    6,6,0,8,3,3,5,5,4,2,2,2,4,4,6,6,
    2,5,0,8,4,4,6,6,2,4,2,7,4,4,7,7,
    6,6,0,8,3,3,5,5,3,2,2,2,3,4,6,6,
    2,5,0,8,4,4,6,6,2,4,2,7,4,4,7,7,
    6,6,0,8,3,3,5,5,4,2,2,2,5,4,6,6,
    2,5,0,8,4,4,6,6,2,4,2,7,4,4,7,7,
    2,6,2,6,3,3,3,3,2,2,2,2,4,4,4,4,
    2,6,0,6,4,4,4,4,2,5,2,5,5,5,5,5,
    2,6,2,6,3,3,3,3,2,2,2,2,4,4,4,4,
    2,5,0,5,4,4,4,4,2,4,2,4,4,4,4,4,
    2,6,2,8,3,3,5,5,2,2,2,2,4,4,6,6,
    2,5,0,8,4,4,6,6,2,4,2,7,4,4,7,7,
    2,6,2,8,3,3,5,5,2,2,2,2,4,4,6,6,
    2,5,0,8,4,4,6,6,2,4,2,7,4,4,7,7,
];

pub struct Cpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub pc: u16,
    pub status: u8,
    cyc: u32,
    extra: u32,
    pub nmi_pending: bool,
    pub irq_pending: bool,
    pub total_cycles: u64,
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            a: 0,
            x: 0,
            y: 0,
            sp: 0xFD,
            pc: 0,
            status: 0x24,
            cyc: 0,
            extra: 0,
            nmi_pending: false,
            irq_pending: false,
            total_cycles: 0,
        }
    }

    pub fn reset(&mut self, bus: &mut Bus) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0xFD;
        self.status = 0x24;
        let lo = bus.read(0xFFFC) as u16;
        let hi = bus.read(0xFFFD) as u16;
        self.pc = lo | (hi << 8);
    }

    fn tick(&mut self, bus: &mut Bus) {
        self.cyc += 1;
        bus.tick();
    }

    fn read(&mut self, bus: &mut Bus, addr: u16) -> u8 {
        let v = bus.read(addr);
        self.tick(bus);
        v
    }

    fn write(&mut self, bus: &mut Bus, addr: u16, val: u8) {
        bus.write(addr, val);
        self.tick(bus);
    }

    fn fetch(&mut self, bus: &mut Bus) -> u8 {
        let v = self.read(bus, self.pc);
        self.pc = self.pc.wrapping_add(1);
        v
    }

    fn fetch16(&mut self, bus: &mut Bus) -> u16 {
        let lo = self.fetch(bus) as u16;
        let hi = self.fetch(bus) as u16;
        lo | (hi << 8)
    }

    fn push(&mut self, bus: &mut Bus, val: u8) {
        self.write(bus, 0x0100 | self.sp as u16, val);
        self.sp = self.sp.wrapping_sub(1);
    }

    fn pop(&mut self, bus: &mut Bus) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        self.read(bus, 0x0100 | self.sp as u16)
    }

    fn set_flag(&mut self, flag: u8, on: bool) {
        if on {
            self.status |= flag;
        } else {
            self.status &= !flag;
        }
    }

    fn get_flag(&self, flag: u8) -> bool {
        self.status & flag != 0
    }

    fn set_zn(&mut self, v: u8) {
        self.set_flag(Z, v == 0);
        self.set_flag(N, v & 0x80 != 0);
    }

    // ----- addressing helpers (return effective address) -----

    fn am_imm(&mut self) -> u16 {
        let a = self.pc;
        self.pc = self.pc.wrapping_add(1);
        a
    }

    fn am_zp(&mut self, bus: &mut Bus) -> u16 {
        self.fetch(bus) as u16
    }

    fn am_zpx(&mut self, bus: &mut Bus) -> u16 {
        self.fetch(bus).wrapping_add(self.x) as u16
    }

    fn am_zpy(&mut self, bus: &mut Bus) -> u16 {
        self.fetch(bus).wrapping_add(self.y) as u16
    }

    fn am_abs(&mut self, bus: &mut Bus) -> u16 {
        self.fetch16(bus)
    }

    fn am_absx(&mut self, bus: &mut Bus) -> (u16, bool) {
        let base = self.fetch16(bus);
        let addr = base.wrapping_add(self.x as u16);
        (addr, (base & 0xFF00) != (addr & 0xFF00))
    }

    fn am_absy(&mut self, bus: &mut Bus) -> (u16, bool) {
        let base = self.fetch16(bus);
        let addr = base.wrapping_add(self.y as u16);
        (addr, (base & 0xFF00) != (addr & 0xFF00))
    }

    fn am_indx(&mut self, bus: &mut Bus) -> u16 {
        let zp = self.fetch(bus).wrapping_add(self.x);
        let lo = self.read(bus, zp as u16) as u16;
        let hi = self.read(bus, zp.wrapping_add(1) as u16) as u16;
        lo | (hi << 8)
    }

    fn am_indy(&mut self, bus: &mut Bus) -> (u16, bool) {
        let zp = self.fetch(bus);
        let lo = self.read(bus, zp as u16) as u16;
        let hi = self.read(bus, zp.wrapping_add(1) as u16) as u16;
        let base = lo | (hi << 8);
        let addr = base.wrapping_add(self.y as u16);
        (addr, (base & 0xFF00) != (addr & 0xFF00))
    }

    // ----- operations -----

    fn adc(&mut self, v: u8) {
        let carry = if self.get_flag(C) { 1u16 } else { 0 };
        let a = self.a as u16;
        let sum = a + v as u16 + carry;
        let result = sum as u8;
        self.set_flag(C, sum > 0xFF);
        self.set_flag(V, (!(a ^ v as u16) & (a ^ sum) & 0x80) != 0);
        self.a = result;
        self.set_zn(result);
    }

    fn sbc(&mut self, v: u8) {
        let carry = if self.get_flag(C) { 1u16 } else { 0 };
        let a = self.a as u16;
        let diff = a.wrapping_sub(v as u16).wrapping_sub(1 - carry);
        let result = diff as u8;
        self.set_flag(C, diff < 0x100);
        self.set_flag(V, ((a ^ v as u16) & (a ^ diff) & 0x80) != 0);
        self.a = result;
        self.set_zn(result);
    }

    fn compare(&mut self, reg: u8, v: u8) {
        self.set_flag(C, reg >= v);
        self.set_zn(reg.wrapping_sub(v));
    }

    fn asl_val(&mut self, v: u8) -> u8 {
        self.set_flag(C, v & 0x80 != 0);
        let r = v << 1;
        self.set_zn(r);
        r
    }

    fn lsr_val(&mut self, v: u8) -> u8 {
        self.set_flag(C, v & 1 != 0);
        let r = v >> 1;
        self.set_zn(r);
        r
    }

    fn rol_val(&mut self, v: u8) -> u8 {
        let carry = if self.get_flag(C) { 1 } else { 0 };
        self.set_flag(C, v & 0x80 != 0);
        let r = (v << 1) | carry;
        self.set_zn(r);
        r
    }

    fn ror_val(&mut self, v: u8) -> u8 {
        let carry = if self.get_flag(C) { 0x80 } else { 0 };
        self.set_flag(C, v & 1 != 0);
        let r = (v >> 1) | carry;
        self.set_zn(r);
        r
    }

    fn branch(&mut self, bus: &mut Bus, cond: bool) {
        let offset = self.fetch(bus) as i8;
        if cond {
            self.extra += 1;
            let old = self.pc;
            self.pc = self.pc.wrapping_add(offset as u16);
            if (old & 0xFF00) != (self.pc & 0xFF00) {
                self.extra += 1;
            }
        }
    }

    pub fn nmi(&mut self, bus: &mut Bus) -> u32 {
        self.cyc = 0;
        let pc = self.pc;
        self.push(bus, (pc >> 8) as u8);
        self.push(bus, (pc & 0xFF) as u8);
        self.push(bus, (self.status | U) & !B);
        self.set_flag(I, true);
        let lo = self.read(bus, 0xFFFA) as u16;
        let hi = self.read(bus, 0xFFFB) as u16;
        self.pc = lo | (hi << 8);
        while self.cyc < 7 {
            self.tick(bus);
        }
        self.total_cycles += self.cyc as u64;
        self.cyc
    }

    pub fn irq(&mut self, bus: &mut Bus) -> u32 {
        self.cyc = 0;
        let pc = self.pc;
        self.push(bus, (pc >> 8) as u8);
        self.push(bus, (pc & 0xFF) as u8);
        self.push(bus, (self.status | U) & !B);
        self.set_flag(I, true);
        let lo = self.read(bus, 0xFFFE) as u16;
        let hi = self.read(bus, 0xFFFF) as u16;
        self.pc = lo | (hi << 8);
        while self.cyc < 7 {
            self.tick(bus);
        }
        self.total_cycles += self.cyc as u64;
        self.cyc
    }

    pub fn step(&mut self, bus: &mut Bus) -> u32 {
        if self.nmi_pending {
            self.nmi_pending = false;
            return self.nmi(bus);
        }
        if self.irq_pending && !self.get_flag(I) {
            self.irq_pending = false;
            return self.irq(bus);
        }

        self.cyc = 0;
        self.extra = 0;
        let opcode = self.fetch(bus);

        match opcode {
            // ---------- ADC ----------
            0x69 => {
                let v = self.read_addr_imm(bus);
                self.adc(v);
            }
            0x65 => {
                let a = self.am_zp(bus);
                let v = self.read(bus, a);
                self.adc(v);
            }
            0x75 => {
                let a = self.am_zpx(bus);
                let v = self.read(bus, a);
                self.adc(v);
            }
            0x6D => {
                let a = self.am_abs(bus);
                let v = self.read(bus, a);
                self.adc(v);
            }
            0x7D => {
                let (a, x) = self.am_absx(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.adc(v);
            }
            0x79 => {
                let (a, x) = self.am_absy(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.adc(v);
            }
            0x61 => {
                let a = self.am_indx(bus);
                let v = self.read(bus, a);
                self.adc(v);
            }
            0x71 => {
                let (a, x) = self.am_indy(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.adc(v);
            }

            // ---------- AND ----------
            0x29 => {
                let v = self.read_addr_imm(bus);
                self.a &= v;
                self.set_zn(self.a);
            }
            0x25 => {
                let a = self.am_zp(bus);
                self.a &= self.read(bus, a);
                self.set_zn(self.a);
            }
            0x35 => {
                let a = self.am_zpx(bus);
                self.a &= self.read(bus, a);
                self.set_zn(self.a);
            }
            0x2D => {
                let a = self.am_abs(bus);
                self.a &= self.read(bus, a);
                self.set_zn(self.a);
            }
            0x3D => {
                let (a, x) = self.am_absx(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.a &= v;
                self.set_zn(self.a);
            }
            0x39 => {
                let (a, x) = self.am_absy(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.a &= v;
                self.set_zn(self.a);
            }
            0x21 => {
                let a = self.am_indx(bus);
                self.a &= self.read(bus, a);
                self.set_zn(self.a);
            }
            0x31 => {
                let (a, x) = self.am_indy(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.a &= v;
                self.set_zn(self.a);
            }

            // ---------- ASL ----------
            0x0A => {
                self.a = self.asl_val(self.a);
            }
            0x06 => {
                let a = self.am_zp(bus);
                let v = self.read(bus, a);
                let r = self.asl_val(v);
                self.write(bus, a, r);
            }
            0x16 => {
                let a = self.am_zpx(bus);
                let v = self.read(bus, a);
                let r = self.asl_val(v);
                self.write(bus, a, r);
            }
            0x0E => {
                let a = self.am_abs(bus);
                let v = self.read(bus, a);
                let r = self.asl_val(v);
                self.write(bus, a, r);
            }
            0x1E => {
                let (a, _) = self.am_absx(bus);
                let v = self.read(bus, a);
                let r = self.asl_val(v);
                self.write(bus, a, r);
            }

            // ---------- branches ----------
            0x90 => self.branch(bus, !self.get_flag(C)),
            0xB0 => self.branch(bus, self.get_flag(C)),
            0xF0 => self.branch(bus, self.get_flag(Z)),
            0x30 => self.branch(bus, self.get_flag(N)),
            0xD0 => self.branch(bus, !self.get_flag(Z)),
            0x10 => self.branch(bus, !self.get_flag(N)),
            0x50 => self.branch(bus, !self.get_flag(V)),
            0x70 => self.branch(bus, self.get_flag(V)),

            // ---------- BIT ----------
            0x24 => {
                let a = self.am_zp(bus);
                let v = self.read(bus, a);
                self.set_flag(Z, self.a & v == 0);
                self.set_flag(N, v & 0x80 != 0);
                self.set_flag(V, v & 0x40 != 0);
            }
            0x2C => {
                let a = self.am_abs(bus);
                let v = self.read(bus, a);
                self.set_flag(Z, self.a & v == 0);
                self.set_flag(N, v & 0x80 != 0);
                self.set_flag(V, v & 0x40 != 0);
            }

            // ---------- BRK / flags ----------
            0x00 => {
                self.fetch(bus);
                let pc = self.pc;
                self.push(bus, (pc >> 8) as u8);
                self.push(bus, (pc & 0xFF) as u8);
                self.push(bus, self.status | B | U);
                self.set_flag(I, true);
                let lo = self.read(bus, 0xFFFE) as u16;
                let hi = self.read(bus, 0xFFFF) as u16;
                self.pc = lo | (hi << 8);
            }
            0x18 => self.set_flag(C, false),
            0x38 => self.set_flag(C, true),
            0x58 => self.set_flag(I, false),
            0x78 => self.set_flag(I, true),
            0xB8 => self.set_flag(V, false),
            0xD8 => self.set_flag(D, false),
            0xF8 => self.set_flag(D, true),

            // ---------- CMP / CPX / CPY ----------
            0xC9 => {
                let v = self.read_addr_imm(bus);
                self.compare(self.a, v);
            }
            0xC5 => {
                let a = self.am_zp(bus);
                let v = self.read(bus, a);
                self.compare(self.a, v);
            }
            0xD5 => {
                let a = self.am_zpx(bus);
                let v = self.read(bus, a);
                self.compare(self.a, v);
            }
            0xCD => {
                let a = self.am_abs(bus);
                let v = self.read(bus, a);
                self.compare(self.a, v);
            }
            0xDD => {
                let (a, x) = self.am_absx(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.compare(self.a, v);
            }
            0xD9 => {
                let (a, x) = self.am_absy(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.compare(self.a, v);
            }
            0xC1 => {
                let a = self.am_indx(bus);
                let v = self.read(bus, a);
                self.compare(self.a, v);
            }
            0xD1 => {
                let (a, x) = self.am_indy(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.compare(self.a, v);
            }
            0xE0 => {
                let v = self.read_addr_imm(bus);
                self.compare(self.x, v);
            }
            0xE4 => {
                let a = self.am_zp(bus);
                let v = self.read(bus, a);
                self.compare(self.x, v);
            }
            0xEC => {
                let a = self.am_abs(bus);
                let v = self.read(bus, a);
                self.compare(self.x, v);
            }
            0xC0 => {
                let v = self.read_addr_imm(bus);
                self.compare(self.y, v);
            }
            0xC4 => {
                let a = self.am_zp(bus);
                let v = self.read(bus, a);
                self.compare(self.y, v);
            }
            0xCC => {
                let a = self.am_abs(bus);
                let v = self.read(bus, a);
                self.compare(self.y, v);
            }

            // ---------- DEC / DEX / DEY ----------
            0xC6 => {
                let a = self.am_zp(bus);
                let v = self.read(bus, a).wrapping_sub(1);
                self.write(bus, a, v);
                self.set_zn(v);
            }
            0xD6 => {
                let a = self.am_zpx(bus);
                let v = self.read(bus, a).wrapping_sub(1);
                self.write(bus, a, v);
                self.set_zn(v);
            }
            0xCE => {
                let a = self.am_abs(bus);
                let v = self.read(bus, a).wrapping_sub(1);
                self.write(bus, a, v);
                self.set_zn(v);
            }
            0xDE => {
                let (a, _) = self.am_absx(bus);
                let v = self.read(bus, a).wrapping_sub(1);
                self.write(bus, a, v);
                self.set_zn(v);
            }
            0xCA => {
                self.x = self.x.wrapping_sub(1);
                self.set_zn(self.x);
            }
            0x88 => {
                self.y = self.y.wrapping_sub(1);
                self.set_zn(self.y);
            }

            // ---------- EOR ----------
            0x49 => {
                let v = self.read_addr_imm(bus);
                self.a ^= v;
                self.set_zn(self.a);
            }
            0x45 => {
                let a = self.am_zp(bus);
                self.a ^= self.read(bus, a);
                self.set_zn(self.a);
            }
            0x55 => {
                let a = self.am_zpx(bus);
                self.a ^= self.read(bus, a);
                self.set_zn(self.a);
            }
            0x4D => {
                let a = self.am_abs(bus);
                self.a ^= self.read(bus, a);
                self.set_zn(self.a);
            }
            0x5D => {
                let (a, x) = self.am_absx(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.a ^= v;
                self.set_zn(self.a);
            }
            0x59 => {
                let (a, x) = self.am_absy(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.a ^= v;
                self.set_zn(self.a);
            }
            0x41 => {
                let a = self.am_indx(bus);
                self.a ^= self.read(bus, a);
                self.set_zn(self.a);
            }
            0x51 => {
                let (a, x) = self.am_indy(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.a ^= v;
                self.set_zn(self.a);
            }

            // ---------- INC / INX / INY ----------
            0xE6 => {
                let a = self.am_zp(bus);
                let v = self.read(bus, a).wrapping_add(1);
                self.write(bus, a, v);
                self.set_zn(v);
            }
            0xF6 => {
                let a = self.am_zpx(bus);
                let v = self.read(bus, a).wrapping_add(1);
                self.write(bus, a, v);
                self.set_zn(v);
            }
            0xEE => {
                let a = self.am_abs(bus);
                let v = self.read(bus, a).wrapping_add(1);
                self.write(bus, a, v);
                self.set_zn(v);
            }
            0xFE => {
                let (a, _) = self.am_absx(bus);
                let v = self.read(bus, a).wrapping_add(1);
                self.write(bus, a, v);
                self.set_zn(v);
            }
            0xE8 => {
                self.x = self.x.wrapping_add(1);
                self.set_zn(self.x);
            }
            0xC8 => {
                self.y = self.y.wrapping_add(1);
                self.set_zn(self.y);
            }

            // ---------- JMP / JSR / RTS / RTI ----------
            0x4C => {
                self.pc = self.fetch16(bus);
            }
            0x6C => {
                let ptr = self.fetch16(bus);
                let lo = self.read(bus, ptr) as u16;
                let hi_addr = (ptr & 0xFF00) | (ptr.wrapping_add(1) & 0x00FF);
                let hi = self.read(bus, hi_addr) as u16;
                self.pc = lo | (hi << 8);
            }
            0x20 => {
                let target = self.fetch16(bus);
                let ret = self.pc.wrapping_sub(1);
                self.push(bus, (ret >> 8) as u8);
                self.push(bus, (ret & 0xFF) as u8);
                self.pc = target;
            }
            0x60 => {
                let lo = self.pop(bus) as u16;
                let hi = self.pop(bus) as u16;
                self.pc = (lo | (hi << 8)).wrapping_add(1);
            }
            0x40 => {
                self.status = (self.pop(bus) & !B) | U;
                let lo = self.pop(bus) as u16;
                let hi = self.pop(bus) as u16;
                self.pc = lo | (hi << 8);
            }

            // ---------- LDA ----------
            0xA9 => {
                self.a = self.read_addr_imm(bus);
                self.set_zn(self.a);
            }
            0xA5 => {
                let a = self.am_zp(bus);
                self.a = self.read(bus, a);
                self.set_zn(self.a);
            }
            0xB5 => {
                let a = self.am_zpx(bus);
                self.a = self.read(bus, a);
                self.set_zn(self.a);
            }
            0xAD => {
                let a = self.am_abs(bus);
                self.a = self.read(bus, a);
                self.set_zn(self.a);
            }
            0xBD => {
                let (a, x) = self.am_absx(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.a = v;
                self.set_zn(self.a);
            }
            0xB9 => {
                let (a, x) = self.am_absy(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.a = v;
                self.set_zn(self.a);
            }
            0xA1 => {
                let a = self.am_indx(bus);
                self.a = self.read(bus, a);
                self.set_zn(self.a);
            }
            0xB1 => {
                let (a, x) = self.am_indy(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.a = v;
                self.set_zn(self.a);
            }

            // ---------- LDX ----------
            0xA2 => {
                self.x = self.read_addr_imm(bus);
                self.set_zn(self.x);
            }
            0xA6 => {
                let a = self.am_zp(bus);
                self.x = self.read(bus, a);
                self.set_zn(self.x);
            }
            0xB6 => {
                let a = self.am_zpy(bus);
                self.x = self.read(bus, a);
                self.set_zn(self.x);
            }
            0xAE => {
                let a = self.am_abs(bus);
                self.x = self.read(bus, a);
                self.set_zn(self.x);
            }
            0xBE => {
                let (a, x) = self.am_absy(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.x = v;
                self.set_zn(self.x);
            }

            // ---------- LDY ----------
            0xA0 => {
                self.y = self.read_addr_imm(bus);
                self.set_zn(self.y);
            }
            0xA4 => {
                let a = self.am_zp(bus);
                self.y = self.read(bus, a);
                self.set_zn(self.y);
            }
            0xB4 => {
                let a = self.am_zpx(bus);
                self.y = self.read(bus, a);
                self.set_zn(self.y);
            }
            0xAC => {
                let a = self.am_abs(bus);
                self.y = self.read(bus, a);
                self.set_zn(self.y);
            }
            0xBC => {
                let (a, x) = self.am_absx(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.y = v;
                self.set_zn(self.y);
            }

            // ---------- LSR ----------
            0x4A => {
                self.a = self.lsr_val(self.a);
            }
            0x46 => {
                let a = self.am_zp(bus);
                let v = self.read(bus, a);
                let r = self.lsr_val(v);
                self.write(bus, a, r);
            }
            0x56 => {
                let a = self.am_zpx(bus);
                let v = self.read(bus, a);
                let r = self.lsr_val(v);
                self.write(bus, a, r);
            }
            0x4E => {
                let a = self.am_abs(bus);
                let v = self.read(bus, a);
                let r = self.lsr_val(v);
                self.write(bus, a, r);
            }
            0x5E => {
                let (a, _) = self.am_absx(bus);
                let v = self.read(bus, a);
                let r = self.lsr_val(v);
                self.write(bus, a, r);
            }

            // ---------- NOP ----------
            0xEA | 0x1A | 0x3A | 0x5A | 0x7A | 0xDA | 0xFA => {}
            0x80 | 0x82 | 0x89 | 0xC2 | 0xE2 => {
                self.fetch(bus);
            }
            0x04 | 0x44 | 0x64 => {
                let a = self.am_zp(bus);
                let _ = self.read(bus, a);
            }
            0x14 | 0x34 | 0x54 | 0x74 | 0xD4 | 0xF4 => {
                let a = self.am_zpx(bus);
                let _ = self.read(bus, a);
            }
            0x0C => {
                let a = self.am_abs(bus);
                let _ = self.read(bus, a);
            }
            0x1C | 0x3C | 0x5C | 0x7C | 0xDC | 0xFC => {
                let (a, x) = self.am_absx(bus);
                let _ = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
            }

            // ---------- ORA ----------
            0x09 => {
                let v = self.read_addr_imm(bus);
                self.a |= v;
                self.set_zn(self.a);
            }
            0x05 => {
                let a = self.am_zp(bus);
                self.a |= self.read(bus, a);
                self.set_zn(self.a);
            }
            0x15 => {
                let a = self.am_zpx(bus);
                self.a |= self.read(bus, a);
                self.set_zn(self.a);
            }
            0x0D => {
                let a = self.am_abs(bus);
                self.a |= self.read(bus, a);
                self.set_zn(self.a);
            }
            0x1D => {
                let (a, x) = self.am_absx(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.a |= v;
                self.set_zn(self.a);
            }
            0x19 => {
                let (a, x) = self.am_absy(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.a |= v;
                self.set_zn(self.a);
            }
            0x01 => {
                let a = self.am_indx(bus);
                self.a |= self.read(bus, a);
                self.set_zn(self.a);
            }
            0x11 => {
                let (a, x) = self.am_indy(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.a |= v;
                self.set_zn(self.a);
            }

            // ---------- stack ----------
            0x48 => self.push(bus, self.a),
            0x68 => {
                self.a = self.pop(bus);
                self.set_zn(self.a);
            }
            0x08 => self.push(bus, self.status | B | U),
            0x28 => {
                self.status = (self.pop(bus) & !B) | U;
            }

            // ---------- ROL ----------
            0x2A => {
                self.a = self.rol_val(self.a);
            }
            0x26 => {
                let a = self.am_zp(bus);
                let v = self.read(bus, a);
                let r = self.rol_val(v);
                self.write(bus, a, r);
            }
            0x36 => {
                let a = self.am_zpx(bus);
                let v = self.read(bus, a);
                let r = self.rol_val(v);
                self.write(bus, a, r);
            }
            0x2E => {
                let a = self.am_abs(bus);
                let v = self.read(bus, a);
                let r = self.rol_val(v);
                self.write(bus, a, r);
            }
            0x3E => {
                let (a, _) = self.am_absx(bus);
                let v = self.read(bus, a);
                let r = self.rol_val(v);
                self.write(bus, a, r);
            }

            // ---------- ROR ----------
            0x6A => {
                self.a = self.ror_val(self.a);
            }
            0x66 => {
                let a = self.am_zp(bus);
                let v = self.read(bus, a);
                let r = self.ror_val(v);
                self.write(bus, a, r);
            }
            0x76 => {
                let a = self.am_zpx(bus);
                let v = self.read(bus, a);
                let r = self.ror_val(v);
                self.write(bus, a, r);
            }
            0x6E => {
                let a = self.am_abs(bus);
                let v = self.read(bus, a);
                let r = self.ror_val(v);
                self.write(bus, a, r);
            }
            0x7E => {
                let (a, _) = self.am_absx(bus);
                let v = self.read(bus, a);
                let r = self.ror_val(v);
                self.write(bus, a, r);
            }

            // ---------- SBC ----------
            0xE9 => {
                let v = self.read_addr_imm(bus);
                self.sbc(v);
            }
            0xE5 => {
                let a = self.am_zp(bus);
                let v = self.read(bus, a);
                self.sbc(v);
            }
            0xF5 => {
                let a = self.am_zpx(bus);
                let v = self.read(bus, a);
                self.sbc(v);
            }
            0xED => {
                let a = self.am_abs(bus);
                let v = self.read(bus, a);
                self.sbc(v);
            }
            0xFD => {
                let (a, x) = self.am_absx(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.sbc(v);
            }
            0xF9 => {
                let (a, x) = self.am_absy(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.sbc(v);
            }
            0xE1 => {
                let a = self.am_indx(bus);
                let v = self.read(bus, a);
                self.sbc(v);
            }
            0xF1 => {
                let (a, x) = self.am_indy(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.sbc(v);
            }

            // ---------- STA ----------
            0x85 => {
                let a = self.am_zp(bus);
                self.write(bus, a, self.a);
            }
            0x95 => {
                let a = self.am_zpx(bus);
                self.write(bus, a, self.a);
            }
            0x8D => {
                let a = self.am_abs(bus);
                self.write(bus, a, self.a);
            }
            0x9D => {
                let (a, _) = self.am_absx(bus);
                self.write(bus, a, self.a);
            }
            0x99 => {
                let (a, _) = self.am_absy(bus);
                self.write(bus, a, self.a);
            }
            0x81 => {
                let a = self.am_indx(bus);
                self.write(bus, a, self.a);
            }
            0x91 => {
                let (a, _) = self.am_indy(bus);
                self.write(bus, a, self.a);
            }

            // ---------- STX / STY ----------
            0x86 => {
                let a = self.am_zp(bus);
                self.write(bus, a, self.x);
            }
            0x96 => {
                let a = self.am_zpy(bus);
                self.write(bus, a, self.x);
            }
            0x8E => {
                let a = self.am_abs(bus);
                self.write(bus, a, self.x);
            }
            0x84 => {
                let a = self.am_zp(bus);
                self.write(bus, a, self.y);
            }
            0x94 => {
                let a = self.am_zpx(bus);
                self.write(bus, a, self.y);
            }
            0x8C => {
                let a = self.am_abs(bus);
                self.write(bus, a, self.y);
            }

            // ---------- transfers ----------
            0xAA => {
                self.x = self.a;
                self.set_zn(self.x);
            }
            0xA8 => {
                self.y = self.a;
                self.set_zn(self.y);
            }
            0x8A => {
                self.a = self.x;
                self.set_zn(self.a);
            }
            0x98 => {
                self.a = self.y;
                self.set_zn(self.a);
            }
            0xBA => {
                self.x = self.sp;
                self.set_zn(self.x);
            }
            0x9A => self.sp = self.x,

            // ---------- unofficial: LAX ----------
            0xA7 => {
                let a = self.am_zp(bus);
                let v = self.read(bus, a);
                self.a = v;
                self.x = v;
                self.set_zn(v);
            }
            0xB7 => {
                let a = self.am_zpy(bus);
                let v = self.read(bus, a);
                self.a = v;
                self.x = v;
                self.set_zn(v);
            }
            0xAF => {
                let a = self.am_abs(bus);
                let v = self.read(bus, a);
                self.a = v;
                self.x = v;
                self.set_zn(v);
            }
            0xBF => {
                let (a, x) = self.am_absy(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.a = v;
                self.x = v;
                self.set_zn(v);
            }
            0xA3 => {
                let a = self.am_indx(bus);
                let v = self.read(bus, a);
                self.a = v;
                self.x = v;
                self.set_zn(v);
            }
            0xB3 => {
                let (a, x) = self.am_indy(bus);
                let v = self.read(bus, a);
                if x {
                    self.extra += 1;
                }
                self.a = v;
                self.x = v;
                self.set_zn(v);
            }

            // ---------- unofficial: SAX ----------
            0x87 => {
                let a = self.am_zp(bus);
                self.write(bus, a, self.a & self.x);
            }
            0x97 => {
                let a = self.am_zpy(bus);
                self.write(bus, a, self.a & self.x);
            }
            0x8F => {
                let a = self.am_abs(bus);
                self.write(bus, a, self.a & self.x);
            }
            0x83 => {
                let a = self.am_indx(bus);
                self.write(bus, a, self.a & self.x);
            }

            _ => {
                // Unknown opcode: treat as 1-byte NOP so the emulator keeps running.
            }
        }

        let base = CYCLES[opcode as usize];
        let target = if base != 0 {
            base as u32 + self.extra
        } else {
            self.cyc + self.extra
        };
        while self.cyc < target {
            self.tick(bus);
        }
        while bus.stall_cycles > 0 {
            bus.stall_cycles -= 1;
            self.tick(bus);
        }
        self.total_cycles += self.cyc as u64;
        self.cyc
    }

    fn read_addr_imm(&mut self, bus: &mut Bus) -> u8 {
        let a = self.am_imm();
        self.read(bus, a)
    }
}
