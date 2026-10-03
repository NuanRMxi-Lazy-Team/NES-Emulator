use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mirroring {
    Horizontal,
    Vertical,
    SingleScreenLower,
    SingleScreenUpper,
    FourScreen,
}

pub trait Mapper {
    fn cpu_read(&self, addr: u16) -> u8;
    fn cpu_write(&mut self, addr: u16, val: u8);
    fn ppu_read(&self, addr: u16) -> u8;
    fn ppu_write(&mut self, addr: u16, val: u8);
    fn mirroring(&self) -> Mirroring;
    fn clock_scanline(&mut self) {}
    fn irq_pending(&self) -> bool {
        false
    }
}

pub type SharedCart = Rc<RefCell<Cartridge>>;

pub struct Cartridge {
    mapper: Box<dyn Mapper>,
}

impl Cartridge {
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() < 16 {
            return Err("ROM too small to contain an iNES header".into());
        }
        if &data[0..4] != b"NES\x1a" {
            return Err("Not a valid iNES ROM (missing NES\\x1a magic)".into());
        }

        let prg_units = data[4] as usize;
        let chr_units = data[5] as usize;
        let flags6 = data[6];
        let flags7 = data[7];

        let mapper_num = (flags6 >> 4) | (flags7 & 0xF0);
        let four_screen = flags6 & 0x08 != 0;
        let has_trainer = flags6 & 0x04 != 0;
        let mirroring = if four_screen {
            Mirroring::FourScreen
        } else if flags6 & 0x01 != 0 {
            Mirroring::Vertical
        } else {
            Mirroring::Horizontal
        };

        let mut offset = 16;
        if has_trainer {
            offset += 512;
        }

        let prg_size = prg_units * 0x4000;
        let chr_size = chr_units * 0x2000;

        if data.len() < offset + prg_size {
            return Err("PRG ROM truncated".into());
        }
        let prg = data[offset..offset + prg_size].to_vec();
        offset += prg_size;

        let (chr, chr_is_ram) = if chr_size == 0 {
            (vec![0u8; 0x2000], true)
        } else {
            if data.len() < offset + chr_size {
                return Err("CHR ROM truncated".into());
            }
            (data[offset..offset + chr_size].to_vec(), false)
        };

        let mapper: Box<dyn Mapper> = match mapper_num {
            0 => Box::new(Nrom {
                prg,
                chr,
                chr_is_ram,
                mirroring,
            }),
            1 => Box::new(Mmc1::new(prg, chr, chr_is_ram, mirroring)),
            2 => Box::new(UxRom {
                prg,
                chr,
                chr_is_ram,
                mirroring,
                bank: 0,
            }),
            3 => Box::new(CnRom {
                prg,
                chr,
                chr_is_ram,
                mirroring,
                bank: 0,
            }),
            4 => Box::new(Mmc3::new(prg, chr, chr_is_ram, mirroring)),
            15 => Box::new(Mmc15::new(prg, chr, chr_is_ram)),
            178 => Box::new(Waixing178::new(prg, chr, chr_is_ram)),
            n => return Err(format!("Unsupported mapper {}", n)),
        };

        Ok(Cartridge { mapper })
    }

    pub fn cpu_read(&self, addr: u16) -> u8 {
        self.mapper.cpu_read(addr)
    }

    pub fn cpu_write(&mut self, addr: u16, val: u8) {
        self.mapper.cpu_write(addr, val)
    }

    pub fn ppu_read(&self, addr: u16) -> u8 {
        self.mapper.ppu_read(addr)
    }

    pub fn ppu_write(&mut self, addr: u16, val: u8) {
        self.mapper.ppu_write(addr, val)
    }

    pub fn mirroring(&self) -> Mirroring {
        self.mapper.mirroring()
    }

    pub fn clock_scanline(&mut self) {
        self.mapper.clock_scanline()
    }

    pub fn irq_pending(&self) -> bool {
        self.mapper.irq_pending()
    }
}

#[cfg(test)]
pub fn test_rom() -> Vec<u8> {
    test_rom_with(&[0xA9, 0x42, 0x8D, 0x00, 0x02, 0x4C, 0x05, 0x80])
}

#[cfg(test)]
pub fn test_rom_vertical() -> Vec<u8> {
    let mut rom = test_rom_with(&[0x4C, 0x00, 0x80]);
    rom[6] = 0x01;
    rom
}

#[cfg(test)]
pub fn test_rom_with(program: &[u8]) -> Vec<u8> {
    let mut rom = vec![0u8; 16 + 0x4000];
    rom[0..4].copy_from_slice(b"NES\x1a");
    rom[4] = 1;
    rom[5] = 0;
    rom[6] = 0;
    rom[16..16 + program.len()].copy_from_slice(program);
    rom[16 + 0x3FFC] = 0x00;
    rom[16 + 0x3FFD] = 0x80;
    rom
}

fn bank_offset(bank: usize, window: usize, rom_len: usize) -> usize {
    if rom_len == 0 {
        return 0;
    }
    (bank * window) % rom_len
}

struct Nrom {
    prg: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    mirroring: Mirroring,
}

impl Mapper for Nrom {
    fn cpu_read(&self, addr: u16) -> u8 {
        if addr < 0x8000 {
            return 0;
        }
        if self.prg.len() == 0x4000 {
            self.prg[(addr as usize - 0x8000) & 0x3FFF]
        } else {
            self.prg[(addr as usize - 0x8000) % self.prg.len()]
        }
    }

    fn cpu_write(&mut self, _addr: u16, _val: u8) {}

    fn ppu_read(&self, addr: u16) -> u8 {
        self.chr[(addr as usize) & 0x1FFF]
    }

    fn ppu_write(&mut self, addr: u16, val: u8) {
        if self.chr_is_ram {
            self.chr[(addr as usize) & 0x1FFF] = val;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }
}

struct UxRom {
    prg: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    mirroring: Mirroring,
    bank: u8,
}

impl Mapper for UxRom {
    fn cpu_read(&self, addr: u16) -> u8 {
        if addr < 0x8000 {
            return 0;
        }
        if addr < 0xC000 {
            let off = bank_offset(self.bank as usize, 0x4000, self.prg.len());
            self.prg[off + (addr as usize & 0x3FFF)]
        } else {
            let off = bank_offset(self.prg.len() / 0x4000 - 1, 0x4000, self.prg.len());
            self.prg[off + (addr as usize & 0x3FFF)]
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8) {
        if addr >= 0x8000 {
            self.bank = val;
        }
    }

    fn ppu_read(&self, addr: u16) -> u8 {
        self.chr[(addr as usize) & 0x1FFF]
    }

    fn ppu_write(&mut self, addr: u16, val: u8) {
        if self.chr_is_ram {
            self.chr[(addr as usize) & 0x1FFF] = val;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }
}

struct CnRom {
    prg: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    mirroring: Mirroring,
    bank: u8,
}

impl Mapper for CnRom {
    fn cpu_read(&self, addr: u16) -> u8 {
        if addr < 0x8000 {
            return 0;
        }
        if self.prg.len() == 0x4000 {
            self.prg[(addr as usize - 0x8000) & 0x3FFF]
        } else {
            self.prg[(addr as usize - 0x8000) % self.prg.len()]
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8) {
        if addr >= 0x8000 {
            self.bank = val & 0x03;
        }
    }

    fn ppu_read(&self, addr: u16) -> u8 {
        let off = bank_offset(self.bank as usize, 0x2000, self.chr.len());
        self.chr[off + (addr as usize & 0x1FFF)]
    }

    fn ppu_write(&mut self, addr: u16, val: u8) {
        if self.chr_is_ram {
            self.chr[(addr as usize) & 0x1FFF] = val;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uxrom_banking() {
        let mut rom = vec![0u8; 16 + 0x20000];
        rom[0..4].copy_from_slice(b"NES\x1a");
        rom[4] = 8;
        rom[5] = 0;
        rom[6] = 0x20;
        for bank in 0..8usize {
            for i in 0..0x4000 {
                rom[16 + bank * 0x4000 + i] = bank as u8;
            }
        }
        let mut cart = Cartridge::from_bytes(&rom).unwrap();
        assert_eq!(cart.cpu_read(0x8000), 0);
        assert_eq!(cart.cpu_read(0xC000), 7);
        cart.cpu_write(0x8000, 5);
        assert_eq!(cart.cpu_read(0x8000), 5);
        assert_eq!(cart.cpu_read(0xBFFF), 5);
        assert_eq!(cart.cpu_read(0xC000), 7);
        cart.cpu_write(0xFFD2, 2);
        assert_eq!(cart.cpu_read(0x8000), 2);
    }

    #[test]
    fn mmc3_banking() {
        let prg_banks = 8usize;
        let chr_banks = 8usize;
        let mut rom = vec![0u8; 16 + prg_banks * 0x2000 + chr_banks * 0x400];
        rom[0..4].copy_from_slice(b"NES\x1a");
        rom[4] = (prg_banks / 2) as u8;
        rom[5] = 1;
        rom[6] = 0x40;
        for b in 0..prg_banks {
            for i in 0..0x2000 {
                rom[16 + b * 0x2000 + i] = b as u8;
            }
        }
        for b in 0..chr_banks {
            for i in 0..0x400 {
                rom[16 + prg_banks * 0x2000 + b * 0x400 + i] = b as u8;
            }
        }
        let mut cart = Cartridge::from_bytes(&rom).unwrap();
        assert_eq!(cart.cpu_read(0x8000), 0);
        assert_eq!(cart.cpu_read(0xC000), 6);
        assert_eq!(cart.cpu_read(0xE000), 7);

        cart.cpu_write(0x8000, 6);
        cart.cpu_write(0x8001, 3);
        assert_eq!(cart.cpu_read(0x8000), 3);
        cart.cpu_write(0x8000, 7);
        cart.cpu_write(0x8001, 5);
        assert_eq!(cart.cpu_read(0xA000), 5);

        cart.cpu_write(0x8000, 0x40);
        assert_eq!(cart.cpu_read(0x8000), 6);
        assert_eq!(cart.cpu_read(0xA000), 3);
        assert_eq!(cart.cpu_read(0xC000), 5);

        cart.cpu_write(0x8000, 0);
        cart.cpu_write(0x8001, 2);
        cart.cpu_write(0x8000, 1);
        cart.cpu_write(0x8001, 4);
        assert_eq!(cart.ppu_read(0x0000), 2);
        assert_eq!(cart.ppu_read(0x0400), 3);
        assert_eq!(cart.ppu_read(0x0800), 4);
        assert_eq!(cart.ppu_read(0x0C00), 5);
    }

    #[test]
    fn mmc3_irq_counter() {
        let mut rom = vec![0u8; 16 + 0x10000];
        rom[0..4].copy_from_slice(b"NES\x1a");
        rom[4] = 4;
        rom[5] = 0;
        rom[6] = 0x40;
        let mut cart = Cartridge::from_bytes(&rom).unwrap();
        cart.cpu_write(0xC000, 3);
        cart.cpu_write(0xC001, 0);
        cart.cpu_write(0xE001, 0);
        for _ in 0..3 {
            cart.clock_scanline();
        }
        assert!(!cart.irq_pending());
        cart.clock_scanline();
        assert!(cart.irq_pending());
        cart.cpu_write(0xE000, 0);
        assert!(!cart.irq_pending());
    }

    #[test]
    fn mmc15_banking() {
        let prg_banks = 16usize;
        let mut rom = vec![0u8; 16 + prg_banks * 0x2000];
        rom[0..4].copy_from_slice(b"NES\x1a");
        rom[4] = (prg_banks / 2) as u8;
        rom[5] = 0;
        rom[6] = 0xF0;
        for b in 0..prg_banks {
            for i in 0..0x2000 {
                rom[16 + b * 0x2000 + i] = b as u8;
            }
        }
        let mut cart = Cartridge::from_bytes(&rom).unwrap();
        assert_eq!(cart.cpu_read(0x8000), 0);
        assert_eq!(cart.cpu_read(0xA000), 1);
        assert_eq!(cart.cpu_read(0xC000), 2);
        assert_eq!(cart.cpu_read(0xE000), 3);

        cart.cpu_write(0x8002, 0x02);
        assert_eq!(cart.cpu_read(0x8000), 4);
        assert_eq!(cart.cpu_read(0xE000), 4);

        cart.cpu_write(0x8000, 0x01);
        assert_eq!(cart.cpu_read(0x8000), 2);
        assert_eq!(cart.cpu_read(0xA000), 3);
        assert_eq!(cart.cpu_read(0xC000), 4);
        assert_eq!(cart.cpu_read(0xE000), 5);

        cart.cpu_write(0x8001, 0x02);
        assert_eq!(cart.cpu_read(0x8000), 4);
        assert_eq!(cart.cpu_read(0xA000), 5);
        assert_eq!(cart.cpu_read(0xC000), 14);
        assert_eq!(cart.cpu_read(0xE000), 15);
    }

    #[test]
    fn waixing178_banking() {
        let prg_banks = 16usize;
        let mut rom = vec![0u8; 16 + prg_banks * 0x4000];
        rom[0..4].copy_from_slice(b"NES\x1a");
        rom[4] = prg_banks as u8;
        rom[5] = 0;
        rom[6] = 0x20;
        rom[7] = 0xB0;
        for b in 0..prg_banks {
            for i in 0..0x4000 {
                rom[16 + b * 0x4000 + i] = b as u8;
            }
        }
        let mut cart = Cartridge::from_bytes(&rom).unwrap();
        assert_eq!(cart.cpu_read(0x8000), 0);
        assert_eq!(cart.cpu_read(0xC000), 1);

        cart.cpu_write(0x4800, 0x02);
        cart.cpu_write(0x4801, 0x03);
        cart.cpu_write(0x4802, 0x01);
        assert_eq!(cart.cpu_read(0x8000), 11);
        assert_eq!(cart.cpu_read(0xC000), 15);

        cart.cpu_write(0x4800, 0x06);
        cart.cpu_write(0x4801, 0x02);
        cart.cpu_write(0x4802, 0x01);
        assert_eq!(cart.cpu_read(0x8000), 10);
        assert_eq!(cart.cpu_read(0xC000), 14);

        cart.cpu_write(0x4800, 0x04);
        cart.cpu_write(0x4801, 0x02);
        cart.cpu_write(0x4802, 0x01);
        assert_eq!(cart.cpu_read(0x8000), 10);
        assert_eq!(cart.cpu_read(0xC000), 10);

        cart.cpu_write(0x4800, 0x00);
        cart.cpu_write(0x4801, 0x02);
        cart.cpu_write(0x4802, 0x01);
        assert_eq!(cart.cpu_read(0x8000), 10);
        assert_eq!(cart.cpu_read(0xC000), 11);

        cart.cpu_write(0x4800, 0x01);
        assert_eq!(cart.mirroring(), Mirroring::Horizontal);

        cart.cpu_write(0x4803, 1);
        cart.cpu_write(0x6000, 0xAB);
        assert_eq!(cart.cpu_read(0x6000), 0xAB);
        cart.cpu_write(0x4803, 2);
        assert_ne!(cart.cpu_read(0x6000), 0xAB);
    }
}

struct Mmc1 {
    prg: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    mirroring: Mirroring,
    shift: u8,
    control: u8,
    chr_bank0: u8,
    chr_bank1: u8,
    prg_bank: u8,
}

impl Mmc1 {
    fn new(prg: Vec<u8>, chr: Vec<u8>, chr_is_ram: bool, mirroring: Mirroring) -> Self {
        Mmc1 {
            prg,
            chr,
            chr_is_ram,
            mirroring,
            shift: 0x10,
            control: 0x0C,
            chr_bank0: 0,
            chr_bank1: 0,
            prg_bank: 0,
        }
    }

    fn prg_read(&self, addr: u16) -> u8 {
        let prg_16k = (self.prg.len() / 0x4000).max(1);
        let mode = (self.control >> 2) & 3;
        let bank = match mode {
            0 | 1 => {
                let b = (self.prg_bank & 0x0E) as usize;
                let slot = if addr < 0xC000 { 0 } else { 1 };
                (b + slot) % (prg_16k * 2)
            }
            2 => {
                if addr < 0xC000 {
                    0
                } else {
                    (self.prg_bank & 0x0F) as usize % prg_16k
                }
            }
            _ => {
                if addr < 0xC000 {
                    (self.prg_bank & 0x0F) as usize % prg_16k
                } else {
                    prg_16k - 1
                }
            }
        };
        let off = bank_offset(bank, 0x4000, self.prg.len());
        self.prg[off + (addr as usize & 0x3FFF)]
    }

    fn chr_read(&self, addr: u16) -> u8 {
        let off = if self.control & 0x10 == 0 {
            let bank = (self.chr_bank0 & 0x1E) as usize / 2;
            bank_offset(bank, 0x2000, self.chr.len())
        } else if addr < 0x1000 {
            bank_offset(self.chr_bank0 as usize, 0x1000, self.chr.len())
        } else {
            bank_offset(self.chr_bank1 as usize, 0x1000, self.chr.len())
        };
        self.chr[off + (addr as usize & 0x0FFF)]
    }

    fn update_mirroring(&mut self) {
        self.mirroring = match self.control & 3 {
            0 => Mirroring::SingleScreenLower,
            1 => Mirroring::SingleScreenUpper,
            2 => Mirroring::Vertical,
            _ => Mirroring::Horizontal,
        };
    }
}

impl Mapper for Mmc1 {
    fn cpu_read(&self, addr: u16) -> u8 {
        if addr >= 0x8000 {
            self.prg_read(addr)
        } else {
            0
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8) {
        if addr < 0x8000 {
            return;
        }
        if val & 0x80 != 0 {
            self.shift = 0x10;
            self.control |= 0x0C;
            self.update_mirroring();
            return;
        }
        let complete = self.shift & 1 != 0;
        self.shift = (self.shift >> 1) | ((val & 1) << 4);
        if complete {
            match (addr >> 13) & 3 {
                0 => {
                    self.control = self.shift;
                    self.update_mirroring();
                }
                1 => self.chr_bank0 = self.shift,
                2 => self.chr_bank1 = self.shift,
                _ => self.prg_bank = self.shift,
            }
            self.shift = 0x10;
        }
    }

    fn ppu_read(&self, addr: u16) -> u8 {
        self.chr_read(addr)
    }

    fn ppu_write(&mut self, addr: u16, val: u8) {
        if self.chr_is_ram {
            let off = if self.control & 0x10 == 0 {
                let bank = (self.chr_bank0 & 0x1E) as usize / 2;
                bank_offset(bank, 0x2000, self.chr.len())
            } else if addr < 0x1000 {
                bank_offset(self.chr_bank0 as usize, 0x1000, self.chr.len())
            } else {
                bank_offset(self.chr_bank1 as usize, 0x1000, self.chr.len())
            };
            self.chr[off + (addr as usize & 0x0FFF)] = val;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }
}

struct Mmc3 {
    prg: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    mirroring: Mirroring,
    bank_select: u8,
    banks: [u8; 8],
    prg_ram: [u8; 0x2000],
    irq_latch: u8,
    irq_counter: u8,
    irq_reload: bool,
    irq_enable: bool,
    irq_pending: bool,
}

impl Mmc3 {
    fn new(prg: Vec<u8>, chr: Vec<u8>, chr_is_ram: bool, mirroring: Mirroring) -> Self {
        Mmc3 {
            prg,
            chr,
            chr_is_ram,
            mirroring,
            bank_select: 0,
            banks: [0; 8],
            prg_ram: [0; 0x2000],
            irq_latch: 0,
            irq_counter: 0,
            irq_reload: false,
            irq_enable: false,
            irq_pending: false,
        }
    }

    fn chr_offset(&self, addr: u16) -> usize {
        let addr = addr & 0x1FFF;
        let (bank, window, base) = if self.bank_select & 0x80 == 0 {
            match addr {
                0x0000..=0x07FF => (self.banks[0] as usize & 0xFE, 0x400, 0x0000),
                0x0800..=0x0FFF => (self.banks[1] as usize & 0xFE, 0x400, 0x0800),
                0x1000..=0x13FF => (self.banks[2] as usize, 0x400, 0x1000),
                0x1400..=0x17FF => (self.banks[3] as usize, 0x400, 0x1400),
                0x1800..=0x1BFF => (self.banks[4] as usize, 0x400, 0x1800),
                _ => (self.banks[5] as usize, 0x400, 0x1C00),
            }
        } else {
            match addr {
                0x0000..=0x03FF => (self.banks[2] as usize, 0x400, 0x0000),
                0x0400..=0x07FF => (self.banks[3] as usize, 0x400, 0x0400),
                0x0800..=0x0BFF => (self.banks[4] as usize, 0x400, 0x0800),
                0x0C00..=0x0FFF => (self.banks[5] as usize, 0x400, 0x0C00),
                0x1000..=0x17FF => (self.banks[0] as usize & 0xFE, 0x400, 0x1000),
                _ => (self.banks[1] as usize & 0xFE, 0x400, 0x1800),
            }
        };
        let off = bank_offset(bank, window, self.chr.len());
        off + (addr as usize - base)
    }
}

impl Mapper for Mmc3 {
    fn cpu_read(&self, addr: u16) -> u8 {
        if addr < 0x6000 {
            return 0;
        }
        if addr < 0x8000 {
            return self.prg_ram[(addr as usize - 0x6000) & 0x1FFF];
        }
        let count = (self.prg.len() / 0x2000).max(1);
        let prg_mode = self.bank_select & 0x40 != 0;
        let bank = match addr {
            0x8000..=0x9FFF => {
                if prg_mode {
                    count.wrapping_sub(2)
                } else {
                    self.banks[6] as usize
                }
            }
            0xA000..=0xBFFF => {
                if prg_mode {
                    self.banks[6] as usize
                } else {
                    self.banks[7] as usize
                }
            }
            0xC000..=0xDFFF => {
                if prg_mode {
                    self.banks[7] as usize
                } else {
                    count.wrapping_sub(2)
                }
            }
            _ => count.wrapping_sub(1),
        };
        let off = bank_offset(bank, 0x2000, self.prg.len());
        self.prg[off + (addr as usize & 0x1FFF)]
    }

    fn cpu_write(&mut self, addr: u16, val: u8) {
        if addr >= 0x8000 {
            let even = addr & 1 == 0;
            match addr & 0xE000 {
                0x8000 => {
                    if even {
                        self.bank_select = val;
                    } else {
                        self.banks[(self.bank_select & 7) as usize] = val;
                    }
                }
                0xA000 => {
                    if even {
                        self.mirroring = if val & 1 == 0 {
                            Mirroring::Vertical
                        } else {
                            Mirroring::Horizontal
                        };
                    }
                }
                0xC000 => {
                    if even {
                        self.irq_latch = val;
                    } else {
                        self.irq_counter = 0;
                        self.irq_reload = true;
                    }
                }
                _ => {
                    if even {
                        self.irq_enable = false;
                        self.irq_pending = false;
                    } else {
                        self.irq_enable = true;
                    }
                }
            }
        } else if addr >= 0x6000 {
            self.prg_ram[(addr as usize - 0x6000) & 0x1FFF] = val;
        }
    }

    fn ppu_read(&self, addr: u16) -> u8 {
        self.chr[self.chr_offset(addr)]
    }

    fn ppu_write(&mut self, addr: u16, val: u8) {
        if self.chr_is_ram {
            let off = self.chr_offset(addr);
            self.chr[off] = val;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }

    fn clock_scanline(&mut self) {
        if self.irq_counter == 0 || self.irq_reload {
            self.irq_counter = self.irq_latch;
            self.irq_reload = false;
        } else {
            self.irq_counter -= 1;
        }
        if self.irq_counter == 0 && self.irq_enable {
            self.irq_pending = true;
        }
    }

    fn irq_pending(&self) -> bool {
        self.irq_pending
    }
}

struct Mmc15 {
    prg: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    mirroring: Mirroring,
    mode: u8,
    prg_banks: [usize; 4],
}

impl Mmc15 {
    fn new(prg: Vec<u8>, chr: Vec<u8>, chr_is_ram: bool) -> Self {
        Mmc15 {
            prg,
            chr,
            chr_is_ram,
            mirroring: Mirroring::Vertical,
            mode: 0,
            prg_banks: [0, 1, 2, 3],
        }
    }

    fn select(&mut self, addr: u16, val: u8) {
        self.mirroring = if val & 0x40 != 0 {
            Mirroring::Horizontal
        } else {
            Mirroring::Vertical
        };
        let sub = (val >> 7) as usize;
        let base = ((val & 0x7F) as usize) << 1;
        self.mode = (addr & 3) as u8;
        match self.mode {
            0 => {
                for i in 0..4 {
                    self.prg_banks[i] = (base + i) ^ sub;
                }
            }
            1 | 3 => {
                let bank = base | sub;
                self.prg_banks[0] = bank;
                self.prg_banks[1] = bank + 1;
                let bank2 = if self.mode == 3 { bank } else { bank | 0x0E } | sub;
                self.prg_banks[2] = bank2;
                self.prg_banks[3] = bank2 + 1;
            }
            2 => {
                let bank = base | sub;
                self.prg_banks = [bank; 4];
            }
            _ => {}
        }
    }
}

impl Mapper for Mmc15 {
    fn cpu_read(&self, addr: u16) -> u8 {
        if addr < 0x8000 {
            return 0;
        }
        let slot = ((addr - 0x8000) / 0x2000) as usize;
        let off = bank_offset(self.prg_banks[slot], 0x2000, self.prg.len());
        self.prg[off + (addr as usize & 0x1FFF)]
    }

    fn cpu_write(&mut self, addr: u16, val: u8) {
        if addr >= 0x8000 {
            self.select(addr, val);
        }
    }

    fn ppu_read(&self, addr: u16) -> u8 {
        self.chr[(addr as usize) & 0x1FFF]
    }

    fn ppu_write(&mut self, addr: u16, val: u8) {
        if self.chr_is_ram && (self.mode == 1 || self.mode == 2) {
            self.chr[(addr as usize) & 0x1FFF] = val;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }
}

struct Waixing178 {
    prg: Vec<u8>,
    chr: Vec<u8>,
    chr_is_ram: bool,
    mirroring: Mirroring,
    regs: [u8; 4],
    work_ram: Vec<u8>,
    prg_banks: [usize; 2],
}

impl Waixing178 {
    fn new(prg: Vec<u8>, chr: Vec<u8>, chr_is_ram: bool) -> Self {
        let mut m = Waixing178 {
            prg,
            chr,
            chr_is_ram,
            mirroring: Mirroring::Vertical,
            regs: [0; 4],
            work_ram: vec![0; 0x8000],
            prg_banks: [0; 2],
        };
        m.update_state();
        m
    }

    fn work_addr(&self, addr: u16) -> usize {
        let page = (self.regs[3] & 0x03) as usize;
        (page * 0x2000 + (addr as usize & 0x1FFF)) % self.work_ram.len()
    }

    fn update_state(&mut self) {
        let sbank = (self.regs[1] & 0x07) as usize;
        let bbank = self.regs[2] as usize;
        if self.regs[0] & 0x02 != 0 {
            self.prg_banks[0] = (bbank << 3) | sbank;
            if self.regs[0] & 0x04 != 0 {
                self.prg_banks[1] = (bbank << 3) | 0x06 | ((self.regs[1] & 0x01) as usize);
            } else {
                self.prg_banks[1] = (bbank << 3) | 0x07;
            }
        } else {
            let bank = (bbank << 3) | sbank;
            if self.regs[0] & 0x04 != 0 {
                self.prg_banks = [bank, bank];
            } else {
                self.prg_banks = [bank, bank + 1];
            }
        }
        self.mirroring = if self.regs[0] & 0x01 != 0 {
            Mirroring::Horizontal
        } else {
            Mirroring::Vertical
        };
    }
}

impl Mapper for Waixing178 {
    fn cpu_read(&self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => self.work_ram[self.work_addr(addr)],
            0x8000..=0xFFFF => {
                let slot = ((addr - 0x8000) / 0x4000) as usize;
                let off = bank_offset(self.prg_banks[slot], 0x4000, self.prg.len());
                self.prg[off + (addr as usize & 0x3FFF)]
            }
            _ => 0,
        }
    }

    fn cpu_write(&mut self, addr: u16, val: u8) {
        if (0x4800..=0x4FFF).contains(&addr) {
            self.regs[(addr & 0x03) as usize] = val;
            self.update_state();
        } else if (0x6000..=0x7FFF).contains(&addr) {
            let off = self.work_addr(addr);
            self.work_ram[off] = val;
        }
    }

    fn ppu_read(&self, addr: u16) -> u8 {
        self.chr[(addr as usize) & 0x1FFF]
    }

    fn ppu_write(&mut self, addr: u16, val: u8) {
        if self.chr_is_ram {
            self.chr[(addr as usize) & 0x1FFF] = val;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }
}
