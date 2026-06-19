#![windows_subsystem = "console"]

use std::{
    env,
    fs::{create_dir_all, read, write},
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use crossterm::{cursor::MoveTo, ExecutableCommand};
use sdl2::{event::Event, keyboard::Keycode, pixels::PixelFormatEnum, rect::Rect};

use gamecrab::core::{
    bus::sram_size_from_rom,
    cpu::{Reg, Reg16},
    emu::{Emu, RegHw},
};

const FREQ: f64 = 4194304.0 / 1.0;
const FAST_FORWARD_FREQ: f64 = FREQ * 2.0;
const PRINT_DEBUG: bool = true;
// const PRINT_INTERVAL: u32 = 1;
const PRINT_INTERVAL: u32 = FREQ as u32 / 240;
const DEBUG_START_FAST_FORWARD_TO: u64 = 0;

const PALETTE: &[(u8, u8, u8)] = &[(255, 255, 255), (170, 170, 170), (85, 85, 85), (0, 0, 0)];

fn main() {
    let sdl = sdl2::init().unwrap();
    let sdl_video = sdl.video().unwrap();
    let window = sdl_video
        .window("gamecrab", 640, 576)
        .opengl()
        .build()
        .unwrap();
    let mut canvas = window
        .into_canvas()
        .index(find_sdl_gl_driver().unwrap())
        .present_vsync()
        .build()
        .unwrap();
    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, 160, 144)
        .unwrap();
    let rom_path = env::args()
        .nth(1)
        .expect("Please provide a ROM path at argument 1.");
    let rom = read(&rom_path).expect("Cannot open file.");
    let (sram, save_path) = load_sram(&rom_path, sram_size_from_rom(&rom));
    let mut emu = Emu::new(rom, sram);
    let uptime = Instant::now();
    let mut last_frame_time = Duration::default();
    let mut freq = FREQ;
    let mut print_debug = PRINT_DEBUG;
    let mut event_pump = sdl.event_pump().unwrap();
    let mut count_to_next_print = 0;
    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::KeyDown {
                    keycode: Some(keycode),
                    ..
                } => match keycode {
                    Keycode::Escape => break 'running,
                    Keycode::Up => emu.bus.borrow_mut().gamepad.up = true,
                    Keycode::Down => emu.bus.borrow_mut().gamepad.down = true,
                    Keycode::Left => emu.bus.borrow_mut().gamepad.left = true,
                    Keycode::Right => emu.bus.borrow_mut().gamepad.right = true,
                    Keycode::A | Keycode::Home => emu.bus.borrow_mut().gamepad.select = true,
                    Keycode::S | Keycode::End => emu.bus.borrow_mut().gamepad.start = true,
                    Keycode::Z | Keycode::PageUp => emu.bus.borrow_mut().gamepad.a = true,
                    Keycode::X | Keycode::PageDown => emu.bus.borrow_mut().gamepad.b = true,
                    Keycode::F => {
                        print_debug = false;
                        freq = FAST_FORWARD_FREQ;
                    }
                    _ => {}
                },
                Event::KeyUp {
                    keycode: Some(keycode),
                    ..
                } => match keycode {
                    Keycode::Up => emu.bus.borrow_mut().gamepad.up = false,
                    Keycode::Down => emu.bus.borrow_mut().gamepad.down = false,
                    Keycode::Left => emu.bus.borrow_mut().gamepad.left = false,
                    Keycode::Right => emu.bus.borrow_mut().gamepad.right = false,
                    Keycode::A | Keycode::Home => emu.bus.borrow_mut().gamepad.select = false,
                    Keycode::S | Keycode::End => emu.bus.borrow_mut().gamepad.start = false,
                    Keycode::Z | Keycode::PageUp => emu.bus.borrow_mut().gamepad.a = false,
                    Keycode::X | Keycode::PageDown => emu.bus.borrow_mut().gamepad.b = false,
                    Keycode::F => {
                        print_debug = PRINT_DEBUG;
                        freq = FREQ;
                    }
                    _ => {}
                },
                Event::MouseButtonDown { .. } => {
                    print_debug = false;
                    freq = FAST_FORWARD_FREQ;
                }
                Event::MouseButtonUp { .. } => {
                    print_debug = PRINT_DEBUG;
                    freq = FREQ;
                }
                _ => {}
            }
        }
        let current_time = uptime.elapsed();
        let delta_time = current_time - last_frame_time;
        last_frame_time = current_time;
        let t_state = emu.clock.borrow().get_t_state();
        let target_t_state = t_state + (delta_time.as_secs_f64() * freq) as u64;
        while emu.clock.borrow().get_t_state() < target_t_state + DEBUG_START_FAST_FORWARD_TO {
            if count_to_next_print <= 0 {
                count_to_next_print = PRINT_INTERVAL;
                _ = io::stdout().execute(MoveTo(0, 0));
                println!(
                    "Clk={}, PC={:04X}, SP={:04X}",
                    emu.clock.borrow().get_t_state(),
                    emu.cpu.get_reg_16(Reg16::PC),
                    emu.cpu.get_reg_16(Reg16::SP),
                );
                if print_debug && t_state >= DEBUG_START_FAST_FORWARD_TO {
                    println!(
                        "AF={:04X}, BC={:04X}, DE={:04X}, HL={:04X}, [HL]={:02X}",
                        emu.cpu.get_reg_16(Reg16::AF),
                        emu.cpu.get_reg_16(Reg16::BC),
                        emu.cpu.get_reg_16(Reg16::DE),
                        emu.cpu.get_reg_16(Reg16::HL),
                        emu.cpu.get_reg(Reg::AddrHL),
                    );
                    println!(
                        "LCDC={:08b}, LY={:03}",
                        emu.bus.borrow().get(RegHw::LCDC as u16),
                        emu.ppu.current_line,
                    );
                    println!(
                        "ROM={}, SRAM={}",
                        emu.bus.borrow().rom_bank,
                        emu.bus.borrow().sram_bank,
                    );
                    print!("Stack   ");
                    {
                        let sp = emu.cpu.get_reg_16(Reg16::SP);
                        let bus = emu.bus.borrow();
                        for addr in sp..(sp + 16) {
                            print!("{:02X} ", bus.get(addr));
                        }
                    }
                    // print!("... \nTiles   "); {
                    //   let bus = emu.bus.borrow();
                    //   for i in 0x8000..0x801B {
                    //     print!("{:02X} ", bus.get(i));
                    //   }
                    // }
                    // println!("...\n\nMap 0 - Visible Area"); {
                    //   let bus = emu.bus.borrow();
                    //   for y in 0..18 {
                    //     for x in 0..20 {
                    //       print!("{:02X} ", bus.get(0x9800 + y * 32 + x));
                    //     }
                    //     println!();
                    //   }
                    // }
                    // println!("\nInstruction Log"); {
                    //   for &(pc, Inst { opcode, operand, operand_16 })
                    //   in emu.cpu.inst_log.iter().take(20) {
                    //     println!("{:04X} {:02X} {:02X} {:04X}",
                    //       pc, opcode, operand, operand_16);
                    //   }
                    // }
                }
            }
            count_to_next_print -= 1;
            emu.tick();
        }
        texture
            .with_lock(None, |buffer, _| {
                for i in 0..(160 * 144) {
                    let (r, g, b) = PALETTE[emu.ppu.framebuffer[i] as usize];
                    buffer[i * 3 + 0] = r;
                    buffer[i * 3 + 1] = g;
                    buffer[i * 3 + 2] = b;
                }
            })
            .unwrap();
        canvas
            .copy(&texture, None, Some(Rect::new(0, 0, 640, 576)))
            .unwrap();
        canvas.present();
    }

    if let (Some(path), Some(sram)) = (save_path, emu.bus.borrow().sram.as_ref()) {
        write(path, sram).unwrap();
    }
}

fn find_sdl_gl_driver() -> Option<u32> {
    for (index, item) in sdl2::render::drivers().enumerate() {
        if item.name == "opengl" {
            return Some(index as u32);
        }
    }
    None
}

fn load_sram(rom_path: &str, sram_size: usize) -> (Option<Vec<u8>>, Option<PathBuf>) {
    if sram_size == 0 {
        return (None, None);
    }

    create_dir_all("save").unwrap();
    let file_name = Path::new(rom_path).file_stem().unwrap().to_str().unwrap();
    let save_path = PathBuf::from(format!("save/{}.sav", file_name));
    let mut sram = read(&save_path).unwrap_or_default();
    sram.resize(sram_size, 0);
    (Some(sram), Some(save_path))
}
