use alloc::{boxed::Box, vec::Vec};
use core::{marker::PhantomData, ptr::NonNull};

use super::{
    bus::Bus,
    clock::Clock,
    cpu::{Cpu, Interrupt},
    ppu::Ppu,
};

pub trait TickProfiler {
    fn now(&mut self) -> u32;

    fn add_cpu(&mut self, _cycles: u32) {}
    fn add_ppu(&mut self, _cycles: u32) {}
    fn add_irq(&mut self, _cycles: u32) {}
    fn add_timer(&mut self, _cycles: u32) {}
    fn add_clock(&mut self, _cycles: u32) {}
    fn add_cpu_tick(&mut self, _executed: bool) {}
    fn add_ppu_tick(&mut self, _line_advanced: bool) {}
    fn add_timer_t_states(&mut self, _t_states: u64) {}
}

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
    bus_storage: Box<Bus>,
    clock_storage: Box<Clock>,
    pub bus: BusHandle,
    pub clock: ClockHandle,
    pub cpu: Cpu,
    pub ppu: Box<Ppu>,
}

impl Emu {
    pub fn new(rom: Vec<u8>, sram: Option<Vec<u8>>) -> Self {
        Self::from_bus(Bus::new(rom, sram))
    }

    pub fn new_static_rom(rom: &'static [u8], sram: Option<Vec<u8>>) -> Self {
        Self::from_bus(Bus::new_static_rom(rom, sram))
    }

    fn from_bus(bus: Bus) -> Self {
        let mut bus_storage = Box::new(bus);
        let mut clock_storage = Box::new(Clock::new());
        let bus_ptr = NonNull::from(bus_storage.as_mut());
        let clock_ptr = NonNull::from(clock_storage.as_mut());
        Self {
            bus_storage,
            clock_storage,
            bus: BusHandle::new(bus_ptr),
            clock: ClockHandle::new(clock_ptr),
            cpu: Cpu::new(bus_ptr, clock_ptr),
            ppu: Box::new(Ppu::new(bus_ptr, clock_ptr)),
        }
    }

    pub fn tick(&mut self) {
        let _ = self.tick_inner(None::<&mut NoopTickProfiler>);
    }

    pub fn tick_profiled(&mut self, profiler: &mut impl TickProfiler) {
        let _ = self.tick_inner(Some(profiler));
    }

    fn tick_inner(&mut self, mut profiler: Option<&mut impl TickProfiler>) -> bool {
        let start = profile_now(&mut profiler);
        let cpu_step = self.cpu.step();
        add_cpu(&mut profiler, start);
        add_cpu_tick(&mut profiler, cpu_step.executed);

        let start = profile_now(&mut profiler);
        let t_state = self.clock_storage.get_t_state() + cpu_step.t_states;
        let mut ppu_line_advanced = false;
        if t_state >= self.ppu.next_line_t_state() {
            while self.ppu.tick_at(t_state) {
                ppu_line_advanced = true;
            }
        }
        add_ppu(&mut profiler, start);
        add_ppu_tick(&mut profiler, ppu_line_advanced);

        let start = profile_now(&mut profiler);
        if self.ppu.irq_vblank {
            self.ppu.irq_vblank = false;
            self.cpu.int_req(Interrupt::VBlank);
        }
        if self.ppu.irq_lcd {
            self.ppu.irq_lcd = false;
            self.cpu.int_req(Interrupt::LCD);
        }
        add_irq(&mut profiler, start);

        let start = profile_now(&mut profiler);
        let timer_irq = {
            let timer = &mut self.bus_storage.timer;
            let mut timer_irq = timer.tick_many(cpu_step.t_states as u16);
            if timer.overflow {
                timer.overflow = false;
                timer_irq = true;
            }
            timer_irq
        };
        add_timer(&mut profiler, start);
        add_timer_t_states(&mut profiler, cpu_step.t_states);

        let start = profile_now(&mut profiler);
        if timer_irq {
            self.cpu.int_req(Interrupt::Timer);
        }
        self.clock_storage.add_t_state(cpu_step.t_states);
        add_clock(&mut profiler, start);

        cpu_step.executed
    }
}

pub struct BusHandle {
    ptr: NonNull<Bus>,
}

impl BusHandle {
    fn new(ptr: NonNull<Bus>) -> Self {
        Self { ptr }
    }

    pub fn borrow(&self) -> BusRef<'_> {
        BusRef {
            ptr: self.ptr,
            marker: PhantomData,
        }
    }

    pub fn borrow_mut(&self) -> BusRefMut<'_> {
        BusRefMut {
            ptr: self.ptr,
            marker: PhantomData,
        }
    }
}

pub struct BusRef<'a> {
    ptr: NonNull<Bus>,
    marker: PhantomData<&'a Bus>,
}

impl core::ops::Deref for BusRef<'_> {
    type Target = Bus;

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}

pub struct BusRefMut<'a> {
    ptr: NonNull<Bus>,
    marker: PhantomData<&'a mut Bus>,
}

impl core::ops::Deref for BusRefMut<'_> {
    type Target = Bus;

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}

impl core::ops::DerefMut for BusRefMut<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ptr.as_mut() }
    }
}

pub struct ClockHandle {
    ptr: NonNull<Clock>,
}

impl ClockHandle {
    fn new(ptr: NonNull<Clock>) -> Self {
        Self { ptr }
    }

    pub fn borrow(&self) -> ClockRef<'_> {
        ClockRef {
            ptr: self.ptr,
            marker: PhantomData,
        }
    }

    pub fn borrow_mut(&self) -> ClockRefMut<'_> {
        ClockRefMut {
            ptr: self.ptr,
            marker: PhantomData,
        }
    }
}

pub struct ClockRef<'a> {
    ptr: NonNull<Clock>,
    marker: PhantomData<&'a Clock>,
}

impl core::ops::Deref for ClockRef<'_> {
    type Target = Clock;

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}

pub struct ClockRefMut<'a> {
    ptr: NonNull<Clock>,
    marker: PhantomData<&'a mut Clock>,
}

impl core::ops::Deref for ClockRefMut<'_> {
    type Target = Clock;

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}

impl core::ops::DerefMut for ClockRefMut<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ptr.as_mut() }
    }
}

struct NoopTickProfiler;

impl TickProfiler for NoopTickProfiler {
    fn now(&mut self) -> u32 {
        0
    }
}

fn profile_now(profiler: &mut Option<&mut impl TickProfiler>) -> u32 {
    match profiler {
        Some(profiler) => profiler.now(),
        None => 0,
    }
}

fn profile_elapsed(profiler: &mut Option<&mut impl TickProfiler>, start: u32) -> u32 {
    match profiler {
        Some(profiler) => profiler.now().wrapping_sub(start),
        None => 0,
    }
}

fn add_cpu(profiler: &mut Option<&mut impl TickProfiler>, start: u32) {
    let elapsed = profile_elapsed(profiler, start);
    if let Some(profiler) = profiler {
        profiler.add_cpu(elapsed);
    }
}

fn add_ppu(profiler: &mut Option<&mut impl TickProfiler>, start: u32) {
    let elapsed = profile_elapsed(profiler, start);
    if let Some(profiler) = profiler {
        profiler.add_ppu(elapsed);
    }
}

fn add_irq(profiler: &mut Option<&mut impl TickProfiler>, start: u32) {
    let elapsed = profile_elapsed(profiler, start);
    if let Some(profiler) = profiler {
        profiler.add_irq(elapsed);
    }
}

fn add_timer(profiler: &mut Option<&mut impl TickProfiler>, start: u32) {
    let elapsed = profile_elapsed(profiler, start);
    if let Some(profiler) = profiler {
        profiler.add_timer(elapsed);
    }
}

fn add_clock(profiler: &mut Option<&mut impl TickProfiler>, start: u32) {
    let elapsed = profile_elapsed(profiler, start);
    if let Some(profiler) = profiler {
        profiler.add_clock(elapsed);
    }
}

fn add_cpu_tick(profiler: &mut Option<&mut impl TickProfiler>, executed: bool) {
    if let Some(profiler) = profiler {
        profiler.add_cpu_tick(executed);
    }
}

fn add_ppu_tick(profiler: &mut Option<&mut impl TickProfiler>, line_advanced: bool) {
    if let Some(profiler) = profiler {
        profiler.add_ppu_tick(line_advanced);
    }
}

fn add_timer_t_states(profiler: &mut Option<&mut impl TickProfiler>, t_states: u64) {
    if let Some(profiler) = profiler {
        profiler.add_timer_t_states(t_states);
    }
}
