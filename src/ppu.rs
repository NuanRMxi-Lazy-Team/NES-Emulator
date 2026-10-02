use crate::cartridge::{Mirroring, SharedCart};

pub const NES_PALETTE: [u32; 64] = [
    0x666666, 0x002A88, 0x1412A7, 0x3B00A4, 0x5C007E, 0x6E0040, 0x6C0600, 0x561D00,
    0x333500, 0x0B4800, 0x005200, 0x004F08, 0x00404D, 0x000000, 0x000000, 0x000000,
    0xADADAD, 0x155FD9, 0x4240FF, 0x7527FE, 0xA01ACC, 0xB71E7B, 0xB53120, 0x994E00,
    0x6B6D00, 0x388700, 0x0C9300, 0x008F32, 0x007C8D, 0x000000, 0x000000, 0x000000,
    0xFFFEFF, 0x64B0FF, 0x9290FF, 0xC676FF, 0xF36AFF, 0xFE6ECC, 0xFE8170, 0xEA9E22,
    0xBCBE00, 0x88D800, 0x5CE430, 0x45E082, 0x48CDDE, 0x4F4F4F, 0x000000, 0x000000,
    0xFFFEFF, 0xC0DFFF, 0xD3D2FF, 0xE8C8FF, 0xFBC2FF, 0xFEC4EA, 0xFECCC5, 0xF7D8A5,
    0xE4E594, 0xCFEF96, 0xBDF4AB, 0xB3F3CC, 0xB5EBF2, 0xB8B8B8, 0x000000, 0x000000,
];

pub struct Ppu {
    pub oam: [u8; 256],
    pub oam_addr: u8,
    pub vram: [u8; 4096],
    pub palette: [u8; 32],
    pub framebuffer: Vec<u32>,

    ctrl: u8,
    mask: u8,
    status: u8,
    v: u16,
    t: u16,
    x: u8,
    w: bool,
    read_buffer: u8,

    pub scanline: i16,
    pub dot: u16,
    odd_frame: bool,
    pub nmi_pending: bool,
    pub frame_ready: bool,
    pub frame: u64,
    pub chr_writes: u64,

    // Background fetch pipeline
    bg_next_tile_id: u8,
    bg_next_tile_attr: u8,
    bg_next_tile_lo: u8,
    bg_next_tile_hi: u8,
    bg_shifter_lo: u16,
    bg_shifter_hi: u16,
    bg_shifter_attr_lo: u16,
    bg_shifter_attr_hi: u16,

    // Sprite pipeline
    sprite_count: usize,
    sp_tile: [u8; 8],
    sp_attr: [u8; 8],
    sp_x: [u8; 8],
    sp_row: [u8; 8],
    sp_is_zero: [bool; 8],
    sp_shift_lo: [u8; 8],
    sp_shift_hi: [u8; 8],
}

impl Ppu {
    pub fn new() -> Self {
        Ppu {
            oam: [0; 256],
            oam_addr: 0,
            vram: [0; 4096],
            palette: [0; 32],
            framebuffer: vec![0; 256 * 240],
            ctrl: 0,
            mask: 0,
            status: 0,
            v: 0,
            t: 0,
            x: 0,
            w: false,
            read_buffer: 0,
            scanline: -1,
            dot: 0,
            odd_frame: false,
            nmi_pending: false,
            frame_ready: false,
            frame: 0,
            chr_writes: 0,
            bg_next_tile_id: 0,
            bg_next_tile_attr: 0,
            bg_next_tile_lo: 0,
            bg_next_tile_hi: 0,
            bg_shifter_lo: 0,
            bg_shifter_hi: 0,
            bg_shifter_attr_lo: 0,
            bg_shifter_attr_hi: 0,
            sprite_count: 0,
            sp_tile: [0; 8],
            sp_attr: [0; 8],
            sp_x: [0; 8],
            sp_row: [0; 8],
            sp_is_zero: [false; 8],
            sp_shift_lo: [0; 8],
            sp_shift_hi: [0; 8],
        }
    }

    pub fn take_nmi(&mut self) -> bool {
        let v = self.nmi_pending;
        self.nmi_pending = false;
        v
    }

    pub fn take_frame_ready(&mut self) -> bool {
        let v = self.frame_ready;
        self.frame_ready = false;
        v
    }

    pub fn regs(&self) -> (u8, u8, u16, u16, u8) {
        (self.ctrl, self.mask, self.v, self.t, self.x)
    }

    fn mirror_addr(&self, addr: u16, cart: &SharedCart) -> usize {
        let table = ((addr - 0x2000) / 0x400) as usize;
        let offset = (addr as usize) & 0x3FF;
        let phys = match cart.borrow().mirroring() {
            Mirroring::Horizontal => (table >> 1) & 1,
            Mirroring::Vertical => table & 1,
            Mirroring::SingleScreenLower => 0,
            Mirroring::SingleScreenUpper => 1,
            Mirroring::FourScreen => table,
        };
        phys * 0x400 + offset
    }

    pub fn ppu_read(&self, addr: u16, cart: &SharedCart) -> u8 {
        let addr = addr & 0x3FFF;
        if addr < 0x2000 {
            cart.borrow().ppu_read(addr)
        } else if addr < 0x3F00 {
            let idx = self.mirror_addr(addr, cart);
            self.vram[idx]
        } else {
            let mut i = (addr & 0x1F) as usize;
            if i & 0x10 != 0 && i & 3 == 0 {
                i -= 0x10;
            }
            self.palette[i]
        }
    }

    pub fn ppu_write(&mut self, addr: u16, val: u8, cart: &SharedCart) {
        let addr = addr & 0x3FFF;
        if addr < 0x2000 {
            self.chr_writes += 1;
            cart.borrow_mut().ppu_write(addr, val);
        } else if addr < 0x3F00 {
            let idx = self.mirror_addr(addr, cart);
            self.vram[idx] = val;
        } else {
            let mut i = (addr & 0x1F) as usize;
            if i & 0x10 != 0 && i & 3 == 0 {
                i -= 0x10;
            }
            self.palette[i] = val;
        }
    }

    pub fn cpu_read(&mut self, reg: u16, cart: &SharedCart) -> u8 {
        match reg & 7 {
            2 => {
                let result = (self.status & 0xE0) | (self.read_buffer & 0x1F);
                self.status &= !0x80;
                self.w = false;
                result
            }
            4 => self.oam[self.oam_addr as usize],
            7 => {
                let addr = self.v;
                let val = self.ppu_read(addr, cart);
                let inc = if self.ctrl & 4 != 0 { 32 } else { 1 };
                self.v = self.v.wrapping_add(inc);
                if addr & 0x3FFF >= 0x3F00 {
                    self.read_buffer = self.ppu_read(addr & 0x2FFF, cart);
                    val & 0x3F
                } else {
                    let result = self.read_buffer;
                    self.read_buffer = val;
                    result
                }
            }
            _ => self.read_buffer,
        }
    }

    pub fn cpu_write(&mut self, reg: u16, val: u8, cart: &SharedCart) {
        match reg & 7 {
            0 => {
                self.ctrl = val;
                self.t = (self.t & !0x0C00) | (((val as u16) & 3) << 10);
            }
            1 => self.mask = val,
            3 => self.oam_addr = val,
            4 => {
                self.oam[self.oam_addr as usize] = val;
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            5 => {
                if !self.w {
                    self.x = val & 7;
                    self.t = (self.t & !0x001F) | ((val as u16) >> 3);
                    self.w = true;
                } else {
                    self.t = (self.t & !0x73E0)
                        | (((val as u16) & 0xF8) << 2)
                        | (((val as u16) & 7) << 12);
                    self.w = false;
                }
            }
            6 => {
                if !self.w {
                    self.t = (self.t & 0x00FF) | (((val as u16) & 0x3F) << 8);
                    self.w = true;
                } else {
                    self.t = (self.t & 0xFF00) | val as u16;
                    self.v = self.t;
                    self.w = false;
                }
            }
            7 => {
                self.ppu_write(self.v, val, cart);
                let inc = if self.ctrl & 4 != 0 { 32 } else { 1 };
                self.v = self.v.wrapping_add(inc);
            }
            _ => {}
        }
    }

    fn increment_x(&mut self) {
        if self.v & 0x001F == 31 {
            self.v &= !0x001F;
            self.v ^= 0x0400;
        } else {
            self.v += 1;
        }
    }

    fn increment_y(&mut self) {
        if (self.v >> 12) & 7 < 7 {
            self.v += 0x1000;
        } else {
            self.v &= !0x7000;
            if self.v & 0x03E0 == 0x03A0 {
                self.v &= !0x03E0;
                self.v ^= 0x0800;
            } else if self.v & 0x03E0 == 0x03E0 {
                self.v &= !0x03E0;
            } else {
                self.v += 0x0020;
            }
        }
    }

    fn copy_x(&mut self) {
        self.v = (self.v & !0x041F) | (self.t & 0x041F);
    }

    fn copy_y(&mut self) {
        self.v = (self.v & !0x7BE0) | (self.t & 0x7BE0);
    }

    fn load_bg_shifters(&mut self) {
        self.bg_shifter_lo = (self.bg_shifter_lo & 0xFF00) | self.bg_next_tile_lo as u16;
        self.bg_shifter_hi = (self.bg_shifter_hi & 0xFF00) | self.bg_next_tile_hi as u16;
        self.bg_shifter_attr_lo = (self.bg_shifter_attr_lo & 0xFF00)
            | if self.bg_next_tile_attr & 1 != 0 { 0xFF } else { 0 };
        self.bg_shifter_attr_hi = (self.bg_shifter_attr_hi & 0xFF00)
            | if self.bg_next_tile_attr & 2 != 0 { 0xFF } else { 0 };
    }

    fn update_shifters(&mut self) {
        if self.mask & 0x08 != 0 {
            self.bg_shifter_lo <<= 1;
            self.bg_shifter_hi <<= 1;
            self.bg_shifter_attr_lo <<= 1;
            self.bg_shifter_attr_hi <<= 1;
        }
        if self.mask & 0x10 != 0 && self.dot >= 2 && self.dot < 258 {
            for i in 0..self.sprite_count {
                if self.sp_x[i] > 0 {
                    self.sp_x[i] -= 1;
                } else {
                    self.sp_shift_lo[i] <<= 1;
                    self.sp_shift_hi[i] <<= 1;
                }
            }
        }
    }

    fn evaluate_sprites(&mut self) {
        self.sprite_count = 0;
        let height: i16 = if self.ctrl & 0x20 != 0 { 16 } else { 8 };
        let next = self.scanline + 1;
        for i in 0..64usize {
            let sy = self.oam[i * 4] as i16;
            let diff = next - sy;
            if diff >= 0 && diff < height {
                if self.sprite_count < 8 {
                    let k = self.sprite_count;
                    self.sp_tile[k] = self.oam[i * 4 + 1];
                    self.sp_attr[k] = self.oam[i * 4 + 2];
                    self.sp_x[k] = self.oam[i * 4 + 3];
                    self.sp_row[k] = diff as u8;
                    self.sp_is_zero[k] = i == 0;
                    self.sprite_count += 1;
                } else {
                    self.status |= 0x20;
                    break;
                }
            }
        }
    }

    fn load_sprite_patterns(&mut self, cart: &SharedCart) {
        let height: u16 = if self.ctrl & 0x20 != 0 { 16 } else { 8 };
        for k in 0..self.sprite_count {
            let tile = self.sp_tile[k];
            let attr = self.sp_attr[k];
            let mut row = self.sp_row[k] as u16;
            if attr & 0x80 != 0 {
                row = height - 1 - row;
            }
            let (table, tile_index): (u16, u16) = if height == 16 {
                let t: u16 = if tile & 1 != 0 { 0x1000 } else { 0 };
                let mut ti = (tile & 0xFE) as u16;
                if row >= 8 {
                    ti += 1;
                }
                (t, ti)
            } else {
                let t: u16 = if self.ctrl & 0x08 != 0 { 0x1000 } else { 0 };
                (t, tile as u16)
            };
            let addr = table + tile_index * 16 + (row & 7);
            let mut lo = self.ppu_read(addr, cart);
            let mut hi = self.ppu_read(addr + 8, cart);
            if attr & 0x40 != 0 {
                lo = lo.reverse_bits();
                hi = hi.reverse_bits();
            }
            self.sp_shift_lo[k] = lo;
            self.sp_shift_hi[k] = hi;
        }
    }

    fn apply_mask(&self, color: u32) -> u32 {
        let mut r = (color >> 16) & 0xFF;
        let mut g = (color >> 8) & 0xFF;
        let mut b = color & 0xFF;
        if self.mask & 0x20 != 0 {
            g = g * 3 / 4;
            b = b * 3 / 4;
        }
        if self.mask & 0x40 != 0 {
            r = r * 3 / 4;
            b = b * 3 / 4;
        }
        if self.mask & 0x80 != 0 {
            r = r * 3 / 4;
            g = g * 3 / 4;
        }
        (r << 16) | (g << 8) | b
    }

    pub fn step(&mut self, cart: &SharedCart) {
        let rendering = self.mask & 0x18 != 0;
        let bg_enabled = self.mask & 0x08 != 0;
        let sp_enabled = self.mask & 0x10 != 0;

        if self.scanline == 241 && self.dot == 1 {
            self.status |= 0x80;
            if self.ctrl & 0x80 != 0 {
                self.nmi_pending = true;
            }
        }
        if self.scanline == -1 && self.dot == 1 {
            self.status &= !0xE0;
        }

        if rendering && self.scanline >= -1 && self.scanline < 240 {
            if (self.dot >= 2 && self.dot < 258) || (self.dot >= 321 && self.dot < 338) {
                self.update_shifters();
                match (self.dot - 1) % 8 {
                    0 => {
                        self.load_bg_shifters();
                        let addr = 0x2000 | (self.v & 0x0FFF);
                        self.bg_next_tile_id = self.ppu_read(addr, cart);
                    }
                    2 => {
                        let addr = 0x23C0
                            | (self.v & 0x0C00)
                            | ((self.v >> 4) & 0x38)
                            | ((self.v >> 2) & 0x07);
                        let mut attr = self.ppu_read(addr, cart);
                        if self.v & 0x0040 != 0 {
                            attr >>= 4;
                        }
                        if self.v & 0x0002 != 0 {
                            attr >>= 2;
                        }
                        self.bg_next_tile_attr = attr & 3;
                    }
                    4 => {
                        let table: u16 = if self.ctrl & 0x10 != 0 { 0x1000 } else { 0 };
                        let addr = table
                            + ((self.bg_next_tile_id as u16) << 4)
                            + ((self.v >> 12) & 0x07);
                        self.bg_next_tile_lo = self.ppu_read(addr, cart);
                    }
                    6 => {
                        let table: u16 = if self.ctrl & 0x10 != 0 { 0x1000 } else { 0 };
                        let addr = table
                            + ((self.bg_next_tile_id as u16) << 4)
                            + ((self.v >> 12) & 0x07)
                            + 8;
                        self.bg_next_tile_hi = self.ppu_read(addr, cart);
                    }
                    7 => self.increment_x(),
                    _ => {}
                }
            }
            if self.dot == 256 {
                self.increment_y();
            }
            if self.dot == 257 {
                self.load_bg_shifters();
                self.copy_x();
            }
            if self.scanline == -1 && self.dot >= 280 && self.dot < 305 {
                self.copy_y();
            }
            if self.dot == 338 || self.dot == 340 {
                let addr = 0x2000 | (self.v & 0x0FFF);
                self.bg_next_tile_id = self.ppu_read(addr, cart);
            }
            if self.dot == 257 {
                self.evaluate_sprites();
            }
            if self.dot == 340 {
                self.load_sprite_patterns(cart);
            }
        }

        if self.scanline >= 0 && self.scanline < 240 && self.dot >= 1 && self.dot <= 256 {
            let px = (self.dot - 1) as usize;
            let bit_mux = 0x8000u16 >> self.x;

            let bg_pixel = {
                let p0 = if self.bg_shifter_lo & bit_mux != 0 { 1u8 } else { 0 };
                let p1 = if self.bg_shifter_hi & bit_mux != 0 { 2u8 } else { 0 };
                if !bg_enabled || (px < 8 && self.mask & 0x02 == 0) {
                    0
                } else {
                    p0 | p1
                }
            };
            let bg_pal = {
                let a0 = if self.bg_shifter_attr_lo & bit_mux != 0 { 1u8 } else { 0 };
                let a1 = if self.bg_shifter_attr_hi & bit_mux != 0 { 2u8 } else { 0 };
                a0 | a1
            };

            let mut sp_pixel = 0u8;
            let mut sp_pal = 0u8;
            let mut sp_priority = false;
            let mut sp_zero = false;
            if sp_enabled && !(px < 8 && self.mask & 0x04 == 0) {
                for i in 0..self.sprite_count {
                    if self.sp_x[i] == 0 {
                        let p0 = if self.sp_shift_lo[i] & 0x80 != 0 { 1u8 } else { 0 };
                        let p1 = if self.sp_shift_hi[i] & 0x80 != 0 { 2u8 } else { 0 };
                        let c = p0 | p1;
                        if c != 0 {
                            sp_pixel = c;
                            sp_pal = (self.sp_attr[i] & 3) + 4;
                            sp_priority = self.sp_attr[i] & 0x20 == 0;
                            sp_zero = self.sp_is_zero[i];
                            break;
                        }
                    }
                }
            }

            if sp_zero && sp_pixel != 0 && bg_pixel != 0 && self.dot != 255 {
                self.status |= 0x40;
            }

            let color = if sp_pixel != 0 && (bg_pixel == 0 || sp_priority) {
                let idx = sp_pal * 4 + sp_pixel;
                NES_PALETTE[self.palette[idx as usize] as usize & 0x3F]
            } else if bg_pixel != 0 {
                let idx = bg_pal * 4 + bg_pixel;
                NES_PALETTE[self.palette[idx as usize] as usize & 0x3F]
            } else {
                NES_PALETTE[self.palette[0] as usize & 0x3F]
            };

            self.framebuffer[(self.scanline as usize) * 256 + px] = self.apply_mask(color);
        }

        self.dot += 1;
        if self.dot >= 341 {
            self.dot = 0;
            self.scanline += 1;
            if self.scanline >= 261 {
                self.scanline = -1;
                self.frame += 1;
                self.frame_ready = true;
                self.odd_frame = !self.odd_frame;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn test_cart() -> SharedCart {
        Rc::new(RefCell::new(
            crate::cartridge::Cartridge::from_bytes(&crate::cartridge::test_rom()).unwrap(),
        ))
    }

    fn run_scanlines(ppu: &mut Ppu, cart: &SharedCart, lines: usize) {
        ppu.scanline = -1;
        ppu.dot = 0;
        for _ in 0..(lines * 341) {
            ppu.step(cart);
        }
    }

    #[test]
    fn renders_background_pattern() {
        let cart = test_cart();
        let mut ppu = Ppu::new();
        ppu.ppu_write(0x0000, 0b1000_0001, &cart);
        ppu.ppu_write(0x0008, 0b0000_0000, &cart);
        ppu.palette[0] = 0x0F;
        ppu.palette[1] = 0x30;
        ppu.cpu_write(1, 0x0A, &cart);
        run_scanlines(&mut ppu, &cart, 2);
        assert_eq!(ppu.framebuffer[0], NES_PALETTE[0x30]);
        assert_eq!(ppu.framebuffer[1], NES_PALETTE[0x0F]);
        assert_eq!(ppu.framebuffer[7], NES_PALETTE[0x30]);
    }

    #[test]
    fn renders_sprite() {
        let cart = test_cart();
        let mut ppu = Ppu::new();
        ppu.ppu_write(0x0000, 0b1000_0001, &cart);
        ppu.ppu_write(0x0008, 0b0000_0000, &cart);
        ppu.palette[0] = 0x0F;
        ppu.palette[0x11] = 0x30;
        ppu.oam[0] = 0;
        ppu.oam[1] = 0;
        ppu.oam[2] = 0;
        ppu.oam[3] = 0;
        ppu.cpu_write(1, 0x14, &cart);
        run_scanlines(&mut ppu, &cart, 2);
        assert_eq!(ppu.framebuffer[0], NES_PALETTE[0x30]);
        assert_eq!(ppu.framebuffer[7], NES_PALETTE[0x30]);
        assert_eq!(ppu.framebuffer[1], NES_PALETTE[0x0F]);
    }

    #[test]
    fn sprite_vertical_position() {
        let cart = test_cart();
        let mut ppu = Ppu::new();
        ppu.ppu_write(0x0000, 0b1000_0001, &cart);
        ppu.palette[0] = 0x0F;
        ppu.palette[0x11] = 0x30;
        ppu.oam[0] = 3;
        ppu.oam[1] = 0;
        ppu.oam[2] = 0;
        ppu.oam[3] = 3;
        ppu.cpu_write(1, 0x14, &cart);
        run_scanlines(&mut ppu, &cart, 6);
        assert_eq!(ppu.framebuffer[2 * 256 + 2], NES_PALETTE[0x0F]);
        assert_eq!(ppu.framebuffer[2 * 256 + 3], NES_PALETTE[0x0F]);
        assert_eq!(ppu.framebuffer[3 * 256 + 2], NES_PALETTE[0x0F]);
        assert_eq!(ppu.framebuffer[3 * 256 + 3], NES_PALETTE[0x30]);
        assert_eq!(ppu.framebuffer[4 * 256 + 3], NES_PALETTE[0x0F]);
    }

    #[test]
    fn v_frozen_when_rendering_disabled() {
        let cart = test_cart();
        let mut ppu = Ppu::new();
        ppu.cpu_write(6, 0x20, &cart);
        ppu.cpu_write(6, 0x01, &cart);
        ppu.scanline = -1;
        ppu.dot = 0;
        for _ in 0..341 {
            ppu.step(&cart);
        }
        assert_eq!(ppu.v, 0x2001);
    }

    #[test]
    fn nametable_seam() {
        let cart = Rc::new(RefCell::new(
            crate::cartridge::Cartridge::from_bytes(&crate::cartridge::test_rom_vertical()).unwrap(),
        ));
        let mut ppu = Ppu::new();
        ppu.ppu_write(0x0010, 0xFF, &cart);
        ppu.ppu_write(0x0018, 0x00, &cart);
        ppu.ppu_write(0x0020, 0xFF, &cart);
        ppu.ppu_write(0x0028, 0xFF, &cart);
        ppu.ppu_write(0x201F, 1, &cart);
        ppu.ppu_write(0x2400, 2, &cart);
        ppu.palette[0] = 0x0F;
        ppu.palette[1] = 0x30;
        ppu.palette[3] = 0x28;
        ppu.cpu_write(1, 0x0A, &cart);
        ppu.cpu_write(5, 0xF8, &cart);
        ppu.cpu_write(5, 0x00, &cart);
        run_scanlines(&mut ppu, &cart, 2);
        assert_eq!(ppu.framebuffer[0], NES_PALETTE[0x30]);
        assert_eq!(ppu.framebuffer[7], NES_PALETTE[0x30]);
        assert_eq!(ppu.framebuffer[8], NES_PALETTE[0x28]);
        assert_eq!(ppu.framebuffer[15], NES_PALETTE[0x28]);
    }

    #[test]
    fn sprite_priority_over_background() {
        for (attr, expected) in [(0x00u8, NES_PALETTE[0x28]), (0x20u8, NES_PALETTE[0x30])] {
            let cart = test_cart();
            let mut ppu = Ppu::new();
            ppu.ppu_write(0x0000, 0xFF, &cart);
            ppu.ppu_write(0x0008, 0x00, &cart);
            ppu.ppu_write(0x0010, 0xFF, &cart);
            ppu.ppu_write(0x0018, 0xFF, &cart);
            ppu.palette[1] = 0x30;
            ppu.palette[0x13] = 0x28;
            ppu.oam[0] = 0;
            ppu.oam[1] = 1;
            ppu.oam[2] = attr;
            ppu.oam[3] = 0;
            ppu.cpu_write(1, 0x1E, &cart);
            run_scanlines(&mut ppu, &cart, 2);
            assert_eq!(ppu.framebuffer[0], expected, "attr={attr:02X}");
        }
    }

    #[test]
    fn uses_attribute_palette() {
        let cart = test_cart();
        let mut ppu = Ppu::new();
        ppu.ppu_write(0x0000, 0xFF, &cart);
        ppu.ppu_write(0x0008, 0x00, &cart);
        ppu.ppu_write(0x23C0, 0x02, &cart);
        ppu.palette[0] = 0x0F;
        ppu.palette[9] = 0x30;
        ppu.cpu_write(1, 0x0A, &cart);
        run_scanlines(&mut ppu, &cart, 2);
        assert_eq!(ppu.framebuffer[0], NES_PALETTE[0x30]);
    }

    #[test]
    fn scrolls_background_vertically() {
        let cart = test_cart();
        let mut ppu = Ppu::new();
        ppu.ppu_write(0x0010, 0xFF, &cart);
        ppu.ppu_write(0x2020, 1, &cart);
        ppu.palette[0] = 0x0F;
        ppu.palette[1] = 0x30;
        ppu.cpu_write(1, 0x0A, &cart);
        ppu.cpu_write(5, 0x00, &cart);
        ppu.cpu_write(5, 0x08, &cart);
        run_scanlines(&mut ppu, &cart, 2);
        assert_eq!(ppu.framebuffer[0], NES_PALETTE[0x30]);
    }

    #[test]
    fn fine_x_scroll() {
        let cart = test_cart();
        let mut ppu = Ppu::new();
        ppu.ppu_write(0x0000, 0b1000_0001, &cart);
        ppu.palette[0] = 0x0F;
        ppu.palette[1] = 0x30;
        ppu.cpu_write(1, 0x0A, &cart);
        ppu.cpu_write(5, 0x03, &cart);
        ppu.cpu_write(5, 0x00, &cart);
        run_scanlines(&mut ppu, &cart, 2);
        assert_eq!(ppu.framebuffer[0], NES_PALETTE[0x0F]);
        assert_eq!(ppu.framebuffer[4], NES_PALETTE[0x30]);
        assert_eq!(ppu.framebuffer[5], NES_PALETTE[0x30]);
    }

    #[test]
    fn scrolls_background() {
        let cart = test_cart();
        let mut ppu = Ppu::new();
        ppu.ppu_write(0x0010, 0xFF, &cart);
        ppu.ppu_write(0x0018, 0x00, &cart);
        ppu.ppu_write(0x2001, 1, &cart);
        ppu.palette[0] = 0x0F;
        ppu.palette[1] = 0x30;
        ppu.cpu_write(1, 0x0A, &cart);
        // Scroll horizontally by one tile using $2005 (fine_x=0, coarse_x=1).
        ppu.cpu_write(5, 0x08, &cart);
        ppu.cpu_write(5, 0x00, &cart);
        run_scanlines(&mut ppu, &cart, 2);
        assert_eq!(ppu.framebuffer[0], NES_PALETTE[0x30]);
        assert_eq!(ppu.framebuffer[7], NES_PALETTE[0x30]);
        assert_eq!(ppu.framebuffer[8], NES_PALETTE[0x0F]);
    }
}
