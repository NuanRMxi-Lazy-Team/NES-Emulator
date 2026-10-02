use crate::apu::Apu;
use crate::cartridge::{Cartridge, SharedCart};
use crate::controller::Controller;
use crate::ppu::Ppu;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Bus {
    pub ram: [u8; 0x800],
    pub ppu: Ppu,
    pub apu: Apu,
    pub cart: SharedCart,
    pub controllers: [Controller; 2],
    pub stall_cycles: u32,
}

impl Bus {
    pub fn new(cart: Cartridge, sample_rate: f64) -> Self {
        Bus {
            ram: [0; 0x800],
            ppu: Ppu::new(),
            apu: Apu::new(sample_rate),
            cart: Rc::new(RefCell::new(cart)),
            controllers: [Controller::new(), Controller::new()],
            stall_cycles: 0,
        }
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize],
            0x2000..=0x3FFF => self.ppu.cpu_read(addr & 7, &self.cart),
            0x4015 => self.apu.read_status(),
            0x4016 => self.controllers[0].read(),
            0x4017 => self.controllers[1].read(),
            0x4018..=0x401F => 0,
            0x4000..=0x4014 => 0,
            _ => self.cart.borrow().cpu_read(addr),
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x07FF) as usize] = val,
            0x2000..=0x3FFF => self.ppu.cpu_write(addr & 7, val, &self.cart),
            0x4014 => self.oam_dma(val),
            0x4016 => {
                self.controllers[0].write(val);
                self.controllers[1].write(val);
            }
            0x4000..=0x4013 | 0x4015 | 0x4017 => self.apu.write(addr, val),
            0x4018..=0x401F => {}
            _ => self.cart.borrow_mut().cpu_write(addr, val),
        }
    }

    fn oam_dma(&mut self, page: u8) {
        let base = (page as u16) << 8;
        let mut buf = [0u8; 256];
        for i in 0..256u16 {
            buf[i as usize] = self.read(base.wrapping_add(i));
        }
        for b in buf.iter() {
            let idx = self.ppu.oam_addr as usize;
            self.ppu.oam[idx] = *b;
            self.ppu.oam_addr = self.ppu.oam_addr.wrapping_add(1);
        }
        self.stall_cycles += 513;
    }

    fn read_dma(&mut self, addr: u16) -> u8 {
        if addr < 0x2000 {
            self.ram[(addr & 0x07FF) as usize]
        } else if addr >= 0x8000 {
            self.cart.borrow().cpu_read(addr)
        } else {
            0
        }
    }

    pub fn tick(&mut self) {
        self.apu.clock();
        if let Some(addr) = self.apu.dmc_dma_request() {
            let val = self.read_dma(addr);
            self.apu.dmc_load(val);
        }
        for _ in 0..3 {
            self.ppu.step(&self.cart);
        }
    }
}
