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
