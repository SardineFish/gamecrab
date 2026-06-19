use alloc::{rc::Rc, vec::Vec};
use core::cell::RefCell;

use super::{
    bus::Bus,
    clock::Clock,
    cpu::{Cpu, Interrupt},
    ppu::Ppu,
};

const T_STATES_PER_TICK: u8 = 4; // Reduce this if accuracy is needed

#[derive(Clone, Copy)]
pub enum RegHw {
    IF = 0xFF0F,
    LCDC = 0xFF40,
    STAT = 0xFF41,
    SCY = 0xFF42,
    SCX = 0xFF43,
    LY = 0xFF44,
    LYC = 0xFF45,
    IE = 0xFFFF,
}

pub struct Emu {
    pub bus: Rc<RefCell<Bus>>,
    pub clock: Rc<RefCell<Clock>>,
    pub cpu: Cpu,
    pub ppu: Ppu,
}

impl Emu {
    pub fn new(rom: Vec<u8>, sram: Option<Vec<u8>>) -> Self {
        let bus = Rc::new(RefCell::new(Bus::new(rom, sram)));
        let clock = Rc::new(RefCell::new(Clock::new()));
        Self {
            bus: bus.clone(),
            clock: clock.clone(),
            cpu: Cpu::new(bus.clone(), clock.clone()),
            ppu: Ppu::new(bus.clone(), clock.clone()),
        }
    }

    pub fn tick(&mut self) {
        let mut timer_irq = false;
        self.cpu.tick();
        self.ppu.tick();
        if self.ppu.irq_vblank {
            self.ppu.irq_vblank = false;
            self.cpu.int_req(Interrupt::VBlank);
        }
        if self.ppu.irq_lcd {
            self.ppu.irq_lcd = false;
            self.cpu.int_req(Interrupt::LCD);
        }
        for _ in 0..T_STATES_PER_TICK {
            let timer = &mut self.bus.borrow_mut().timer;
            timer.tick();
            if timer.overflow {
                timer.overflow = false;
                timer_irq = true;
            }
        }
        if timer_irq {
            self.cpu.int_req(Interrupt::Timer);
        }
        self.clock.borrow_mut().add_t_state(T_STATES_PER_TICK);
    }
}
