mod gamepad;
pub mod oam;
mod timer;

use memmap2::{Mmap, MmapMut};

use self::{
    gamepad::{Gamepad, GamepadRegion},
    oam::Oam,
    timer::Timer,
};

enum CartType {
    ROM,
    MBC1,
    MBC3,
    MBC5,
}

pub struct Bus {
    pub rom: Mmap,
    pub vram: [u8; 0x2000],
    pub sram: Option<MmapMut>,
    pub wram: [u8; 0x2000],
    pub oam: Oam,
    pub io: [u8; 0x80],
    pub hram: [u8; 0x7F],
    pub ie: u8,
    pub rom_bank: u16,
    pub sram_bank: u8,
    // TODO: write locks
    pub vram_lock: bool,
    pub oam_lock: bool,
    pub dma_transferring: bool,
    pub gamepad: Gamepad,
    pub timer: Timer,
    pub serial_output: Vec<u8>,
    cart_type: CartType,
}

impl Bus {
    pub fn new(rom: Mmap, sram: Option<MmapMut>) -> Self {
        let cart_type = match rom[0x147] {
            0x00 => CartType::ROM,
            0x01 => CartType::MBC1,
            0x02 => CartType::MBC1,
            0x03 => CartType::MBC1,
            0x0F => CartType::MBC3,
            0x10 => CartType::MBC3,
            0x11 => CartType::MBC3,
            0x12 => CartType::MBC3,
            0x13 => CartType::MBC3,
            0x19 => CartType::MBC5,
            0x1A => CartType::MBC5,
            0x1B => CartType::MBC5,
            0x1C => CartType::MBC5,
            0x1D => CartType::MBC5,
            0x1E => CartType::MBC5,
            _ => unimplemented!("Unsupported cartridge type"),
        };
        Self {
            rom,
            vram: [0; 0x2000],
            sram,
            wram: [0; 0x2000],
            oam: Oam::new(),
            io: [0; 0x80],
            hram: [0; 0x7F],
            ie: 0,
            rom_bank: 1,
            sram_bank: 0,
            vram_lock: false,
            oam_lock: false,
            dma_transferring: false,
            gamepad: Gamepad::new(),
            timer: Timer::new(),
            serial_output: Vec::new(),
            cart_type,
        }
    }
    pub fn get(&self, addr: u16) -> u8 {
        let idx = addr as usize;
        match addr {
            0x0000..=0x3FFF => self.rom[idx - 0x0000],
            0x4000..=0x7FFF => self.rom[idx - 0x4000 + self.rom_bank as usize * 0x4000],
            0x8000..=0x9FFF => self.vram[idx - 0x8000],
            0xA000..=0xBFFF => match &self.sram {
                Some(sram) => sram[0x2000 * self.sram_bank as usize + idx - 0xA000],
                None => 0,
            },
            0xC000..=0xDFFF => self.wram[idx - 0xC000],
            0xE000..=0xFDFF => self.wram[idx - 0xE000],
            0xFE00..=0xFE9F => self.oam.get(addr as u8),
            0xFEA0..=0xFEFF => 0xFF,
            0xFF00 => self.gamepad.get(),
            0xFF01..=0xFF03 => self.io[idx - 0xFF00],
            0xFF04..=0xFF07 => self.timer.get(addr as u8 - 4),
            0xFF08..=0xFF7F => self.io[idx - 0xFF00],
            0xFF80..=0xFFFE => self.hram[idx - 0xFF80],
            0xFFFF => self.ie,
        }
    }
    pub fn set(&mut self, addr: u16, mut value: u8) {
        value = mask(addr, value);
        let idx = addr as usize;
        match addr {
            0x0000..=0x7FFF => match self.cart_type {
                CartType::ROM => {}
                CartType::MBC1 => match addr {
                    0x0000..=0x1FFF => {}
                    0x2000..=0x3FFF => {
                        self.rom_bank = if value & 0x1F == 0 {
                            1
                        } else {
                            value as u16 & 0x1F
                        }
                    }
                    0x4000..=0x5FFF => self.sram_bank = value & 0x3,
                    0x6000..=0x7FFF => {}
                    _ => unreachable!(),
                },
                CartType::MBC3 => match addr {
                    0x0000..=0x1FFF => {}
                    0x2000..=0x3FFF => {
                        self.rom_bank = if value & 0x7F == 0 {
                            1
                        } else {
                            value as u16 & 0x7F
                        }
                    }
                    0x4000..=0x5FFF => self.sram_bank = value & 0x3,
                    0x6000..=0x7FFF => {}
                    _ => unreachable!(),
                },
                CartType::MBC5 => match addr {
                    0x0000..=0x1FFF => {}
                    0x2000..=0x2FFF => self.rom_bank = (self.rom_bank & 0xFF00) | value as u16,
                    0x3000..=0x3FFF => {
                        self.rom_bank = (self.rom_bank & 0x00FF) | (value as u16 & 1) << 8
                    }
                    0x4000..=0x5FFF => self.sram_bank = value & 0xF,
                    0x6000..=0x7FFF => {}
                    _ => unreachable!(),
                },
            },
            0x8000..=0x9FFF => {
                if !self.vram_lock {
                    self.vram[idx - 0x8000] = value;
                }
            }
            0xA000..=0xBFFF => match &mut self.sram {
                Some(sram) => sram[0x2000 * self.sram_bank as usize + idx - 0xA000] = value,
                None => {}
            },
            0xC000..=0xDFFF => self.wram[idx - 0xC000] = value,
            0xE000..=0xFDFF => self.wram[idx - 0xE000] = value,
            0xFE00..=0xFE9F => {
                if !self.oam_lock {
                    self.oam.set(addr as u8, value);
                }
            }
            0xFEA0..=0xFEFF => {}
            0xFF00 => match value >> 4 & 0b_11 {
                0 => self.gamepad.region = GamepadRegion::Buttons,
                1 => self.gamepad.region = GamepadRegion::Buttons,
                2 => self.gamepad.region = GamepadRegion::DPad,
                3 => self.gamepad.region = GamepadRegion::None,
                _ => unreachable!(),
            },
            0xFF01..=0xFF03 => {
                self.io[idx - 0xFF00] = value;
                if addr == 0xFF02 && value == 0x81 {
                    self.serial_output.push(self.io[0x01]);
                }
            }
            0xFF04..=0xFF07 => self.timer.set(addr as u8 - 4, value),
            0xFF08..=0xFF45 => self.io[idx - 0xFF00] = value,
            0xFF46 => {
                self.io[idx - 0xFF00] = value;
                self.dma((value as u16) << 8);
            }
            0xFF47..=0xFF7F => self.io[idx - 0xFF00] = value,
            0xFF80..=0xFFFE => self.hram[idx - 0xFF80] = value,
            0xFFFF => self.ie = value,
        }
    }
    pub fn lock_vram(&mut self) {
        self.vram_lock = true;
    }
    pub fn unlock_vram(&mut self) {
        self.vram_lock = false;
    }
    pub fn lock_oam(&mut self) {
        self.oam_lock = true;
    }
    pub fn unlock_oam(&mut self) {
        self.oam_lock = false;
    }
    fn dma(&mut self, addr: u16) {
        for i in 0..160 {
            self.oam.set(i, self.get(addr + i as u16));
        }
    }
}

fn mask(addr: u16, value: u8) -> u8 {
    match addr {
        0xFF0F | 0xFFFF => value | 0xE0,
        _ => value,
    }
}
