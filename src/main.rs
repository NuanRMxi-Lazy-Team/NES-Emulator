mod apu;
mod audio;
mod bus;
mod cartridge;
mod controller;
mod cpu;
mod emulator;
mod input;
mod ppu;

use audio::Audio;
use emulator::Emulator;
use input::{
    Input, BTN_A, BTN_B, BTN_DOWN, BTN_LEFT, BTN_RIGHT, BTN_SELECT, BTN_START, BTN_UP,
};
use minifb::{Key, Scale, Window, WindowOptions};
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

fn run_dump(rom_path: &str, frames: u64, out_path: &str) -> Result<(), String> {
    let rom = std::fs::read(rom_path).map_err(|e| format!("read {rom_path}: {e}"))?;
    let mut emu = Emulator::new(&rom, 44100.0)?;
    for f in 0..frames {
        let mut b = 0u8;
        if (100..103).contains(&f) {
            b |= BTN_START;
        }
        if (140..143).contains(&f) {
            b |= BTN_A;
        }
        if f >= 200 {
            b |= BTN_RIGHT;
            if f % 12 < 3 {
                b |= BTN_B;
            }
        }
        emu.set_buttons(b, 0);
        emu.run_frame();
    }
    let fb = emu.framebuffer();
    let file = File::create(out_path).map_err(|e| format!("create {out_path}: {e}"))?;
    let w = BufWriter::new(file);
    let mut enc = png::Encoder::new(w, 256, 240);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    let mut data = Vec::with_capacity(256 * 240 * 3);
    for &px in fb {
        data.push(((px >> 16) & 0xFF) as u8);
        data.push(((px >> 8) & 0xFF) as u8);
        data.push((px & 0xFF) as u8);
    }
    writer.write_image_data(&data).map_err(|e| e.to_string())?;
    println!("wrote {out_path}");
    if std::env::var("NES_INK").is_err() {
        return Ok(());
    }
    print_ascii(fb);
    let ppu = &emu.bus.ppu;
    let (ctrl, mask, v, t, x) = ppu.regs();
    println!(
        "ctrl={:02X} mask={:02X} v={:04X} t={:04X} x={} chr_writes={}",
        ctrl, mask, v, t, x, ppu.chr_writes
    );
    print!("palette:");
    for p in ppu.palette.iter() {
        print!(" {:02X}", p);
    }
    println!();
    for y in 0..240 {
        let mut line = String::with_capacity(256);
        for x in 0..256 {
            let px = fb[y * 256 + x];
            let lum = ((px >> 16) & 0xFF) + ((px >> 8) & 0xFF) + (px & 0xFF);
            line.push(if lum > 90 { '#' } else { '.' });
        }
        println!("{line}");
    }
    Ok(())
}

fn print_ascii(fb: &[u32]) {
    let chars = b" .:-=+*#%@";
    for y in (0..240).step_by(4) {
        let mut line = String::new();
        for x in (0..256).step_by(2) {
            let mut sum = 0u32;
            let mut cnt = 0u32;
            for dy in 0..4 {
                for dx in 0..2 {
                    let px = fb[(y + dy) * 256 + (x + dx)];
                    let r = (px >> 16) & 0xFF;
                    let g = (px >> 8) & 0xFF;
                    let b = px & 0xFF;
                    sum += (r + g + b) / 3;
                    cnt += 1;
                }
            }
            let lum = sum / cnt;
            let idx = (lum as usize * (chars.len() - 1)) / 255;
            line.push(chars[idx] as char);
        }
        println!("{line}");
    }
}

fn read_keyboard(window: &Window, turbo: bool) -> u8 {
    let mut b = 0u8;
    if window.is_key_down(Key::Z) {
        b |= BTN_A;
    }
    if window.is_key_down(Key::X) {
        b |= BTN_B;
    }
    if window.is_key_down(Key::A) && turbo {
        b |= BTN_A;
    }
    if window.is_key_down(Key::S) && turbo {
        b |= BTN_B;
    }
    if window.is_key_down(Key::Up) {
        b |= BTN_UP;
    }
    if window.is_key_down(Key::Down) {
        b |= BTN_DOWN;
    }
    if window.is_key_down(Key::Left) {
        b |= BTN_LEFT;
    }
    if window.is_key_down(Key::Right) {
        b |= BTN_RIGHT;
    }
    if window.is_key_down(Key::Enter) {
        b |= BTN_START;
    }
    if window.is_key_down(Key::RightShift) || window.is_key_down(Key::LeftShift) {
        b |= BTN_SELECT;
    }
    b
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 5 && args[1] == "--dump" {
        let frames = args[3].parse::<u64>().unwrap_or(120);
        if let Err(e) = run_dump(&args[2], frames, &args[4]) {
            eprintln!("dump failed: {e}");
            std::process::exit(1);
        }
        return;
    }
    if args.len() < 2 {
        eprintln!("Usage: {} <rom.nes> [window_scale 1-4]", args[0]);
        eprintln!("       {} --dump <rom.nes> <frames> <out.png>", args[0]);
        std::process::exit(1);
    }

    let path = PathBuf::from(&args[1]);
    let rom = match std::fs::read(&path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to read {}: {e}", path.display());
            std::process::exit(1);
        }
    };

    let scale = args
        .get(2)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(3);
    let scale = match scale {
        1 => Scale::X1,
        2 => Scale::X2,
        3 => Scale::X4,
        4 => Scale::X8,
        _ => Scale::X4,
    };

    let audio = Audio::new();

    let mut emu = match Emulator::new(&rom, audio.sample_rate as f64) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Failed to load ROM: {e}");
            std::process::exit(1);
        }
    };

    let mut window = match Window::new(
        "NES Emulator",
        256,
        240,
        WindowOptions {
            scale,
            resize: true,
            ..Default::default()
        },
    ) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Failed to create window: {e}");
            std::process::exit(1);
        }
    };

    let mut input = Input::new();
    if input.is_none() {
        eprintln!("No gamepad backend available; keyboard only.");
    }

    println!("Loaded {}.", path.display());
    println!("Gamepad (XInput): D-Pad = Move, A/B = A/B, X/Y = Turbo A/B, Start/Select = Start/Select");
    println!("Keyboard: Arrows = Move, Z/X = A/B, A/S = Turbo A/B, Enter = Start, Shift = Select");
    println!("Press Esc to quit.");

    let mut kframe: u32 = 0;
    let frame_time = std::time::Duration::from_micros(16_667);
    let mut last = std::time::Instant::now();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        kframe = kframe.wrapping_add(1);
        let kturbo = (kframe / 3) % 2 == 0;

        let mut buttons = 0u8;
        if let Some(i) = input.as_mut() {
            buttons |= i.poll();
        }
        buttons |= read_keyboard(&window, kturbo);

        emu.set_buttons(buttons, 0);
        emu.run_frame();

        let samples = emu.take_audio();
        audio.push(&samples);

        if window
            .update_with_buffer(emu.framebuffer(), 256, 240)
            .is_err()
        {
            break;
        }

        if audio.has_output() {
            // Audio-driven pacing: keep a cushion (~50 ms) in the output buffer so the
            // hardware callback never runs dry (which would cause popping).
            let target = (audio.sample_rate as i64) / 20;
            let level = audio.buffered() as i64;
            let diff = level - target;
            if diff > 0 {
                let us = (diff * 1_000_000 / audio.sample_rate as i64).clamp(0, 20_000);
                std::thread::sleep(std::time::Duration::from_micros(us as u64));
            }
        } else {
            let elapsed = last.elapsed();
            if elapsed < frame_time {
                std::thread::sleep(frame_time - elapsed);
            }
            last = std::time::Instant::now();
        }
    }
}
