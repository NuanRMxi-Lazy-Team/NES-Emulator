use crate::bus::Bus;
use crate::cartridge::Cartridge;
use crate::cpu::Cpu;

#[allow(dead_code)]
pub const CPU_FREQ: f64 = 1789773.0;

pub struct Emulator {
    pub cpu: Cpu,
    pub bus: Bus,
}

impl Emulator {
    pub fn new(rom: &[u8], sample_rate: f64) -> Result<Self, String> {
        let cart = Cartridge::from_bytes(rom)?;
        let bus = Bus::new(cart, sample_rate);
        let mut emu = Emulator {
            cpu: Cpu::new(),
            bus,
        };
        emu.cpu.reset(&mut emu.bus);
        for _ in 0..2 {
            emu.run_frame();
        }
        emu.bus.ppu.take_nmi();
        Ok(emu)
    }

    pub fn run_frame(&mut self) {
        self.bus.ppu.take_frame_ready();
        while !self.bus.ppu.take_frame_ready() {
            if self.bus.cart.borrow().irq_pending() {
                self.cpu.irq_pending = true;
            }
            self.cpu.step(&mut self.bus);
            if self.bus.ppu.take_nmi() {
                self.cpu.nmi_pending = true;
            }
        }
    }

    pub fn framebuffer(&self) -> &[u32] {
        &self.bus.ppu.framebuffer
    }

    pub fn set_buttons(&mut self, player1: u8, player2: u8) {
        self.bus.controllers[0].buttons = player1;
        self.bus.controllers[1].buttons = player2;
    }

    pub fn take_audio(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.bus.apu.output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_executes_program() {
        let rom = crate::cartridge::test_rom();
        let mut emu = Emulator::new(&rom, 44100.0).expect("rom loads");
        emu.run_frame();
        assert_eq!(emu.bus.ram[0x200], 0x42);
    }

    #[test]
    fn arithmetic_works() {
        let program: &[u8] = &[
            0xA9, 0x05, 0x18, 0x69, 0x03, 0x8D, 0x00, 0x02, 0xA9, 0x10, 0x38, 0xE9, 0x01, 0x8D,
            0x01, 0x02, 0x4C, 0x10, 0x80,
        ];
        let rom = crate::cartridge::test_rom_with(program);
        let mut emu = Emulator::new(&rom, 44100.0).expect("rom loads");
        emu.run_frame();
        assert_eq!(emu.bus.ram[0x200], 8);
        assert_eq!(emu.bus.ram[0x201], 0x0F);
    }

    #[test]
    fn copies_memory_with_indexed_addressing() {
        let program: &[u8] = &[
            0xA2, 0x00, 0x8A, 0x9D, 0x00, 0x03, 0xE8, 0xE0, 0x10, 0xD0, 0xF7, 0xA2, 0x00, 0xBD,
            0x00, 0x03, 0x9D, 0x00, 0x04, 0xE8, 0xE0, 0x10, 0xD0, 0xF5, 0x4C, 0x18, 0x80,
        ];
        let rom = crate::cartridge::test_rom_with(program);
        let mut emu = Emulator::new(&rom, 44100.0).expect("rom loads");
        emu.run_frame();
        for i in 0..16usize {
            assert_eq!(emu.bus.ram[0x400 + i], i as u8, "copy index {i}");
        }
    }

    #[test]
    fn runs_frames_without_panic() {
        let rom = crate::cartridge::test_rom();
        let mut emu = Emulator::new(&rom, 44100.0).expect("rom loads");
        for _ in 0..5 {
            emu.run_frame();
        }
        assert_eq!(emu.framebuffer().len(), 256 * 240);
    }
}
