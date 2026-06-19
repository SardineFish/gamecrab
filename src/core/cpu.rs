#[cfg(feature = "cpu-trace")]
use alloc::collections::BTreeSet;
use alloc::{collections::VecDeque, rc::Rc};
use core::cell::RefCell;

use super::{bus::Bus, clock::Clock, emu::RegHw};

use Flag::{C as CF, H as HF, N as NF, Z as ZF};
use Reg::*;
use Reg16::*;

static INST_LENGTH: &[u8] = &[
    1, 3, 1, 1, 1, 1, 2, 1, 3, 1, 1, 1, 1, 1, 2, 1, 2, 3, 1, 1, 1, 1, 2, 1, 2, 1, 1, 1, 1, 1, 2, 1,
    2, 3, 1, 1, 1, 1, 2, 1, 2, 1, 1, 1, 1, 1, 2, 1, 2, 3, 1, 1, 1, 1, 2, 1, 2, 1, 1, 1, 1, 1, 2, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 3, 3, 3, 1, 2, 1, 1, 1, 3, 2, 3, 3, 2, 1, 1, 1, 3, 0, 3, 1, 2, 1, 1, 1, 3, 0, 3, 0, 2, 1,
    2, 1, 1, 0, 0, 1, 2, 1, 2, 1, 3, 0, 0, 0, 2, 1, 2, 1, 1, 1, 0, 1, 2, 1, 2, 1, 3, 1, 0, 0, 2, 1,
];

static INST_BASE_CYCLES: &[u8] = &[
    1, 3, 2, 2, 1, 1, 2, 1, 5, 2, 2, 2, 1, 1, 2, 1, 1, 3, 2, 2, 1, 1, 2, 1, 2, 2, 2, 2, 1, 1, 2, 1,
    2, 3, 2, 2, 1, 1, 2, 1, 2, 2, 2, 2, 1, 1, 2, 1, 2, 3, 2, 2, 3, 3, 3, 1, 2, 2, 2, 2, 1, 1, 2, 1,
    1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1,
    1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1, 2, 2, 2, 2, 2, 2, 1, 2, 1, 1, 1, 1, 1, 1, 2, 1,
    1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1,
    1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1,
    2, 3, 3, 3, 3, 4, 2, 4, 2, 4, 3, 2, 3, 3, 2, 4, 2, 3, 3, 0, 3, 4, 2, 4, 2, 4, 3, 0, 3, 0, 2, 4,
    3, 3, 2, 0, 0, 4, 2, 4, 4, 1, 4, 0, 0, 0, 2, 4, 3, 3, 2, 1, 0, 4, 2, 4, 3, 2, 4, 1, 0, 0, 2, 4,
];

#[derive(Clone, Copy)]
pub enum Reg {
    B,
    C,
    D,
    E,
    H,
    L,
    AddrHL,
    A,
    AddrBC,
    AddrDE,
    F,
    Imm8(u8),
}

#[derive(Clone, Copy)]
pub enum Reg16 {
    BC,
    DE,
    HL,
    AF,
    SP,
    PC,
}

#[derive(Clone, Copy)]
pub enum Flag {
    Z,
    N,
    H,
    C,
}

#[derive(Clone, Copy)]
pub enum Interrupt {
    VBlank,
    LCD,
    Timer,
    Serial,
    Joypad,
}

#[derive(Clone, Copy, Default)]
pub struct Inst {
    pub opcode: u8,
    pub operand: u8,
    pub operand_16: u16,
}

pub struct Cpu {
    bus: Rc<RefCell<Bus>>,
    clock: Rc<RefCell<Clock>>,
    a: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
    f: u8,
    sp: u16,
    pc: u16,
    ime: bool,
    ei_pending: bool,
    halting: bool,
    next_inst_t_state: u64,
    pub inst_log: VecDeque<(u16, Inst)>,
    #[cfg(feature = "cpu-trace")]
    trace: BTreeSet<u32>,
}

impl Cpu {
    pub fn new(bus: Rc<RefCell<Bus>>, clock: Rc<RefCell<Clock>>) -> Self {
        Self {
            bus,
            clock,
            a: 0x01,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            f: 0b_10000000,
            sp: 0xFFFE,
            pc: 0x0100,
            ime: false,
            ei_pending: false,
            halting: false,
            next_inst_t_state: 0,
            inst_log: VecDeque::with_capacity(20),
            #[cfg(feature = "cpu-trace")]
            trace: BTreeSet::new(),
        }
    }

    pub fn get_reg(&self, reg: Reg) -> u8 {
        match reg {
            A => self.a,
            B => self.b,
            C => self.c,
            D => self.d,
            E => self.e,
            H => self.h,
            L => self.l,
            F => self.f,
            AddrBC => self.bus.borrow().get(self.get_reg_16(BC)),
            AddrDE => self.bus.borrow().get(self.get_reg_16(DE)),
            AddrHL => self.bus.borrow().get(self.get_reg_16(HL)),
            Imm8(value) => value,
        }
    }
    fn set_reg(&mut self, reg: Reg, value: u8) {
        match reg {
            A => self.a = value,
            B => self.b = value,
            C => self.c = value,
            D => self.d = value,
            E => self.e = value,
            H => self.h = value,
            L => self.l = value,
            F => self.f = value & 0xF0,
            AddrBC => self.bus.borrow_mut().set(self.get_reg_16(BC), value),
            AddrDE => self.bus.borrow_mut().set(self.get_reg_16(DE), value),
            AddrHL => self.bus.borrow_mut().set(self.get_reg_16(HL), value),
            Imm8(_) => panic!(),
        }
    }
    pub fn get_reg_16(&self, reg: Reg16) -> u16 {
        match reg {
            AF => self.reg_pair_to_u16(A, F),
            BC => self.reg_pair_to_u16(B, C),
            DE => self.reg_pair_to_u16(D, E),
            HL => self.reg_pair_to_u16(H, L),
            SP => self.sp,
            PC => self.pc,
        }
    }
    fn set_reg_16(&mut self, reg: Reg16, value: u16) {
        match reg {
            AF => self.u16_to_reg_pair(A, F, value),
            BC => self.u16_to_reg_pair(B, C, value),
            DE => self.u16_to_reg_pair(D, E, value),
            HL => self.u16_to_reg_pair(H, L, value),
            SP => self.sp = value,
            PC => self.pc = value,
        }
    }
    fn reg_pair_to_u16(&self, hi: Reg, lo: Reg) -> u16 {
        self.get_reg(lo) as u16 | (self.get_reg(hi) as u16) << 8
    }
    fn u16_to_reg_pair(&mut self, hi: Reg, lo: Reg, value: u16) {
        self.set_reg(hi, (value >> 8) as u8);
        self.set_reg(lo, (value >> 0) as u8);
    }

    pub fn get_flag(&self, flag: Flag) -> bool {
        self.get_reg(F) & get_flag_mask(flag) > 0
    }
    fn set_flag(&mut self, flag: Flag, value: bool) {
        if value {
            self.set_reg(F, self.get_reg(F) | get_flag_mask(flag))
        } else {
            self.set_reg(F, self.get_reg(F) & !get_flag_mask(flag))
        }
    }

    fn push(&mut self, value: u16) {
        let sp = self.get_reg_16(SP);
        self.bus.borrow_mut().set(sp - 1, (value >> 8) as u8);
        self.bus.borrow_mut().set(sp - 2, (value >> 0) as u8);
        self.set_reg_16(SP, sp - 2);
    }
    fn pop(&mut self) -> u16 {
        let sp = self.get_reg_16(SP);
        self.set_reg_16(SP, sp.wrapping_add(2));
        self.bus.borrow().get(sp) as u16 | (self.bus.borrow().get(sp.wrapping_add(1)) as u16) << 8
    }

    fn add(&mut self, reg: Reg) {
        let lhs = self.get_reg(A);
        let rhs = self.get_reg(reg);
        let result = lhs.wrapping_add(rhs);
        self.set_reg(A, result);
        self.set_flag(ZF, result == 0);
        self.set_flag(NF, false);
        self.set_flag(HF, (lhs & 0b_1111) + (rhs & 0b_1111) > 0b_1111);
        self.set_flag(CF, lhs as u16 + rhs as u16 > 0xFF);
    }
    fn adc(&mut self, reg: Reg) {
        let lhs = self.get_reg(A);
        let rhs = self.get_reg(reg);
        let carry = self.get_flag(CF) as u8;
        let result = lhs.wrapping_add(rhs).wrapping_add(carry);
        self.set_reg(A, result);
        self.set_flag(ZF, result == 0);
        self.set_flag(NF, false);
        self.set_flag(HF, (lhs & 0b_1111) + (rhs & 0b_1111) + carry > 0b_1111);
        self.set_flag(CF, lhs as u16 + rhs as u16 + carry as u16 > 0xFF);
    }
    fn sub(&mut self, reg: Reg) {
        let lhs = self.get_reg(A);
        let rhs = self.get_reg(reg);
        let result = lhs.wrapping_sub(rhs);
        self.set_reg(A, result);
        self.set_flag(ZF, result == 0);
        self.set_flag(NF, true);
        self.set_flag(HF, lhs & 0b_1111 < rhs & 0b_1111);
        self.set_flag(CF, (lhs as u16) < rhs as u16);
    }
    fn sbc(&mut self, reg: Reg) {
        let lhs = self.get_reg(A);
        let rhs = self.get_reg(reg);
        let carry = self.get_flag(CF) as u8;
        let result = lhs.wrapping_sub(rhs).wrapping_sub(carry);
        self.set_reg(A, result);
        self.set_flag(ZF, result == 0);
        self.set_flag(NF, true);
        self.set_flag(HF, lhs & 0b_1111 < ((rhs & 0b_1111) + carry));
        self.set_flag(CF, (lhs as u16) < rhs as u16 + carry as u16);
    }
    fn and(&mut self, reg: Reg) {
        let result = self.get_reg(A) & self.get_reg(reg);
        self.set_reg(A, result);
        self.set_flag(ZF, result == 0);
        self.set_flag(NF, false);
        self.set_flag(HF, true);
        self.set_flag(CF, false);
    }
    fn xor(&mut self, reg: Reg) {
        let result = self.get_reg(A) ^ self.get_reg(reg);
        self.set_reg(A, result);
        self.set_flag(ZF, result == 0);
        self.set_flag(NF, false);
        self.set_flag(HF, false);
        self.set_flag(CF, false);
    }
    fn or(&mut self, reg: Reg) {
        let result = self.get_reg(A) | self.get_reg(reg);
        self.set_reg(A, result);
        self.set_flag(ZF, result == 0);
        self.set_flag(NF, false);
        self.set_flag(HF, false);
        self.set_flag(CF, false);
    }
    fn cp(&mut self, reg: Reg) {
        let lhs = self.get_reg(A);
        let rhs = self.get_reg(reg);
        let result = lhs.wrapping_sub(rhs);
        self.set_flag(ZF, result == 0);
        self.set_flag(NF, true);
        self.set_flag(HF, lhs & 0b_1111 < rhs & 0b_1111);
        self.set_flag(CF, lhs < rhs);
    }

    fn inc(&mut self, reg: Reg) {
        let result = self.get_reg(reg).wrapping_add(1);
        self.set_reg(reg, result);
        self.set_flag(ZF, result == 0);
        self.set_flag(NF, false);
        self.set_flag(HF, result & 0b_1111 == 0b_0000);
    }
    fn dec(&mut self, reg: Reg) {
        let result = self.get_reg(reg).wrapping_sub(1);
        self.set_reg(reg, result);
        self.set_flag(ZF, result == 0);
        self.set_flag(NF, true);
        self.set_flag(HF, result & 0b_1111 == 0b_1111);
    }

    fn rlc(&mut self, reg: Reg) {
        let value = self.get_reg(reg);
        let result = value.rotate_left(1);
        self.set_reg(reg, result);
        self.set_flag(ZF, result == 0);
        self.set_flag(NF, false);
        self.set_flag(HF, false);
        self.set_flag(CF, value & 0b_10000000 > 0);
    }
    fn rrc(&mut self, reg: Reg) {
        let value = self.get_reg(reg);
        let result = value.rotate_right(1);
        self.set_reg(reg, result);
        self.set_flag(ZF, result == 0);
        self.set_flag(NF, false);
        self.set_flag(HF, false);
        self.set_flag(CF, value & 0b_00000001 > 0);
    }
    fn rl(&mut self, reg: Reg) {
        let value = self.get_reg(reg);
        let result = value << 1 | self.get_flag(CF) as u8;
        self.set_reg(reg, result);
        self.set_flag(ZF, result == 0);
        self.set_flag(NF, false);
        self.set_flag(HF, false);
        self.set_flag(CF, value & 0b_10000000 > 0);
    }
    fn rr(&mut self, reg: Reg) {
        let value = self.get_reg(reg);
        let result = value >> 1 | (self.get_flag(CF) as u8) << 7;
        self.set_reg(reg, result);
        self.set_flag(ZF, result == 0);
        self.set_flag(NF, false);
        self.set_flag(HF, false);
        self.set_flag(CF, value & 0b_00000001 > 0);
    }
    fn sla(&mut self, reg: Reg) {
        let value = self.get_reg(reg);
        let result = value << 1;
        self.set_reg(reg, result);
        self.set_flag(ZF, result == 0);
        self.set_flag(NF, false);
        self.set_flag(HF, false);
        self.set_flag(CF, value & 0b_10000000 > 0);
    }
    fn sra(&mut self, reg: Reg) {
        let value = self.get_reg(reg);
        let result = (value as i8 >> 1) as u8;
        self.set_reg(reg, result);
        self.set_flag(ZF, result == 0);
        self.set_flag(NF, false);
        self.set_flag(HF, false);
        self.set_flag(CF, value & 0b_00000001 > 0);
    }
    fn swap(&mut self, reg: Reg) {
        let result = self.get_reg(reg).rotate_left(4);
        self.set_reg(reg, result);
        self.set_flag(ZF, result == 0);
        self.set_flag(NF, false);
        self.set_flag(HF, false);
        self.set_flag(CF, false);
    }
    fn srl(&mut self, reg: Reg) {
        let value = self.get_reg(reg);
        let result = value >> 1;
        self.set_reg(reg, result);
        self.set_flag(ZF, result == 0);
        self.set_flag(NF, false);
        self.set_flag(HF, false);
        self.set_flag(CF, value & 0b_00000001 > 0);
    }

    fn bit_test(&mut self, reg: Reg, bit: u8) {
        self.set_flag(ZF, self.get_reg(reg) & 1 << bit == 0);
        self.set_flag(NF, false);
        self.set_flag(HF, true);
    }
    fn bit_reset(&mut self, reg: Reg, bit: u8) {
        self.set_reg(reg, self.get_reg(reg) & !(1 << bit));
    }
    fn bit_set(&mut self, reg: Reg, bit: u8) {
        self.set_reg(reg, self.get_reg(reg) | 1 << bit);
    }

    fn add_16(&mut self, reg: Reg16) {
        let lhs = self.get_reg_16(HL);
        let rhs = self.get_reg_16(reg);
        let result = lhs.wrapping_add(rhs);
        self.set_reg_16(HL, result);
        self.set_flag(NF, false);
        self.set_flag(HF, (lhs & 0xFFF) + (rhs & 0xFFF) > 0xFFF);
        self.set_flag(CF, lhs as u32 + rhs as u32 > 0xFFFF);
    }
    fn inc_16(&mut self, reg: Reg16) {
        self.set_reg_16(reg, self.get_reg_16(reg).wrapping_add(0x0001));
    }
    fn dec_16(&mut self, reg: Reg16) {
        self.set_reg_16(reg, self.get_reg_16(reg).wrapping_sub(0x0001));
    }

    fn jr(&mut self, offset: u8) {
        let lhs = self.get_reg_16(PC);
        self.set_reg_16(PC, lhs.wrapping_add_signed(offset as i8 as i16));
    }
    fn jp(&mut self, addr: u16) {
        self.set_reg_16(PC, addr);
    }

    fn next_byte(&mut self) -> u8 {
        let pc = self.get_reg_16(PC);
        let byte = self.bus.borrow().get(pc);
        self.set_reg_16(PC, pc.wrapping_add(1));
        byte
    }
    fn next_inst(&mut self) -> Inst {
        let pc = self.get_reg_16(PC);
        let opcode = self.next_byte();
        let inst = match INST_LENGTH[opcode as usize] {
            1 => Inst {
                opcode,
                ..Default::default()
            },
            2 => Inst {
                opcode,
                operand: self.next_byte(),
                ..Default::default()
            },
            3 => Inst {
                opcode,
                operand_16: self.next_byte() as u16 | (self.next_byte() as u16) << 8,
                ..Default::default()
            },
            _ => panic!("Last address: 0x{:X}\nOpcode: {:X}", self.pc, opcode),
        };
        self.inst_log.push_back((pc, inst));
        if self.inst_log.len() > 20 {
            self.inst_log.pop_front();
        }
        inst
    }

    pub fn tick(&mut self) {
        if self.clock.borrow().get_t_state() < self.next_inst_t_state {
            return;
        }
        if self.halting {
            self.delay(1);
            return;
        }
        let mut ir = self.bus.borrow().get(RegHw::IF as u16);
        ir &= 0x1F;
        ir &= self.bus.borrow().get(RegHw::IE as u16);
        if self.ime && ir > 0 {
            let int_id = 7 - ir.leading_zeros();
            self.bus
                .borrow_mut()
                .set(RegHw::IF as u16, ir & !(1 << int_id));
            self.ime = false;
            self.push(self.get_reg_16(PC));
            self.jp(0x40 + int_id as u16 * 8);
            self.delay(5);
        }
        if self.ei_pending {
            self.ime = true;
            self.ei_pending = false;
        }
        if self.pc == 0x5FE6 {
            self.halting = false;
        }
        self.trace_pc();
        let Inst {
            opcode,
            operand,
            operand_16,
        } = self.next_inst();
        match opcode {
            0x00 => {}
            0x01 | 0x11 | 0x21 | 0x31 => {
                self.set_reg_16(id_to_reg_16(opcode >> 4), operand_16);
            }
            0x02 | 0x0A | 0x12 | 0x1A | 0x22 | 0x2A | 0x32 | 0x3A => {
                let reg = [AddrBC, AddrDE, AddrHL, AddrHL][opcode as usize >> 4];
                let (dst, src) = if opcode & 0b_1000 > 0 {
                    (A, reg)
                } else {
                    (reg, A)
                };
                self.set_reg(dst, self.get_reg(src));
                match opcode >> 4 {
                    2 => self.inc_16(HL),
                    3 => self.dec_16(HL),
                    _ => {}
                }
            }
            0x03 | 0x0B | 0x13 | 0x1B | 0x23 | 0x2B | 0x33 | 0x3B => {
                let reg = id_to_reg_16(opcode >> 4);
                if opcode & 0b_1000 > 0 {
                    self.dec_16(reg);
                } else {
                    self.inc_16(reg);
                };
            }
            0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => {
                self.inc(id_to_reg(opcode >> 3));
            }
            0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
                self.dec(id_to_reg(opcode >> 3));
            }
            0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x36 | 0x3E => {
                self.set_reg(id_to_reg(opcode >> 3), operand);
            }
            0x07 | 0x0F | 0x17 | 0x1F => {
                let f = [Self::rlc, Self::rrc, Self::rl, Self::rr][opcode as usize >> 3];
                f(self, A);
                self.set_flag(ZF, false);
            }
            0x08 => {
                self.bus
                    .borrow_mut()
                    .set(operand_16 + 0, (self.get_reg_16(SP) >> 0) as u8);
                self.bus
                    .borrow_mut()
                    .set(operand_16 + 1, (self.get_reg_16(SP) >> 8) as u8);
            }
            0x09 | 0x19 | 0x29 | 0x39 => self.add_16(id_to_reg_16(opcode >> 4)),
            0x10 => {}
            0x18 | 0x20 | 0x28 | 0x30 | 0x38 => {
                let z = self.get_flag(ZF);
                let c = self.get_flag(CF);
                let cond = [true, !z, z, !c, c][opcode as usize - 0x18 >> 3];
                if cond {
                    self.jr(operand);
                    self.delay(1);
                }
            }
            0x27 => {
                let mut result = self.get_reg(A) as u16;
                if self.get_flag(NF) {
                    if self.get_flag(HF) {
                        result = result.wrapping_sub(6);
                        if !self.get_flag(CF) {
                            result &= 0xFF;
                        }
                    }
                    if self.get_flag(CF) {
                        result = result.wrapping_sub(0x60);
                    }
                } else {
                    if self.get_flag(HF) || result & 0xF > 9 {
                        result += 0x06;
                    }
                    if self.get_flag(CF) || result > 0x9F {
                        result += 0x60;
                    }
                }
                self.set_reg(A, result as u8);
                self.set_flag(ZF, result as u8 == 0);
                self.set_flag(HF, false);
                self.set_flag(CF, self.get_flag(CF) || result > 0xFF);
            }
            0x2F => {
                self.set_reg(A, !self.get_reg(A));
                self.set_flag(NF, true);
                self.set_flag(HF, true);
            }
            0x37 | 0x3F => {
                self.set_flag(NF, false);
                self.set_flag(HF, false);
                if opcode == 0x37 {
                    self.set_flag(CF, true);
                } else {
                    self.set_flag(CF, !self.get_flag(CF));
                }
            }
            0x40..=0x7F => {
                if opcode == 0x76 {
                    self.halting = true;
                } else {
                    let dst = id_to_reg(opcode - 0x40 >> 3);
                    let src = id_to_reg(opcode & 0b_111);
                    self.set_reg(dst, self.get_reg(src));
                }
            }
            0x80..=0xBF => {
                let alu_op = id_to_alu_op(opcode - 0x80 >> 3);
                alu_op(self, id_to_reg(opcode & 0b_111));
            }
            0xC0 | 0xC8 | 0xC9 | 0xD0 | 0xD8 | 0xD9 => match opcode {
                0xC9 => {
                    let stack_top = self.pop();
                    self.set_reg_16(PC, stack_top);
                }
                0xD9 => {
                    self.ei_pending = true;
                    let stack_top = self.pop();
                    self.set_reg_16(PC, stack_top);
                }
                _ => {
                    let z = self.get_flag(ZF);
                    let c = self.get_flag(CF);
                    let cond = [!z, z, !c, c][opcode as usize - 0xC0 >> 3];
                    if cond {
                        let stack_top = self.pop();
                        self.set_reg_16(PC, stack_top);
                        self.delay(3);
                    }
                }
            },
            0xC1 | 0xC5 | 0xD1 | 0xD5 | 0xE1 | 0xE5 | 0xF1 | 0xF5 => {
                let reg = [BC, DE, HL, AF][opcode as usize - 0xC0 >> 4];
                if opcode & 0b_100 > 0 {
                    self.push(self.get_reg_16(reg));
                } else {
                    let value = self.pop();
                    self.set_reg_16(reg, value);
                }
            }
            0xC2 | 0xC3 | 0xCA | 0xD2 | 0xDA | 0xE9 => {
                let cond = match opcode {
                    0xC3 | 0xE9 => true,
                    0xC2 => !self.get_flag(ZF),
                    0xCA => self.get_flag(ZF),
                    0xD2 => !self.get_flag(CF),
                    0xDA => self.get_flag(CF),
                    _ => unreachable!(),
                };
                if cond {
                    if opcode == 0xE9 {
                        self.jp(self.get_reg_16(HL));
                    } else {
                        self.jp(operand_16);
                        self.delay(1);
                    }
                }
            }
            0xC4 | 0xCC | 0xCD | 0xD4 | 0xDC => {
                let cond = match opcode {
                    0xCD => true,
                    0xC4 => !self.get_flag(ZF),
                    0xCC => self.get_flag(ZF),
                    0xD4 => !self.get_flag(CF),
                    0xDC => self.get_flag(CF),
                    _ => unreachable!(),
                };
                if cond {
                    self.push(self.get_reg_16(PC));
                    self.jp(operand_16);
                    self.delay(3);
                }
            }
            0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE => {
                id_to_alu_op(opcode - 0xC0 >> 3)(self, Imm8(operand));
            }
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
                self.push(self.get_reg_16(PC));
                self.jp(opcode as u16 - 0xC7);
            }
            0xCB => {
                let reg = id_to_reg(operand & 0b_111);
                match operand {
                    0x00..=0x07 => self.rlc(reg),
                    0x08..=0x0F => self.rrc(reg),
                    0x10..=0x17 => self.rl(reg),
                    0x18..=0x1F => self.rr(reg),
                    0x20..=0x27 => self.sla(reg),
                    0x28..=0x2F => self.sra(reg),
                    0x30..=0x37 => self.swap(reg),
                    0x38..=0x3F => self.srl(reg),
                    0x40..=0x7F => self.bit_test(reg, operand - 0x40 >> 3),
                    0x80..=0xBF => self.bit_reset(reg, operand - 0x80 >> 3),
                    0xC0..=0xFF => self.bit_set(reg, operand - 0xC0 >> 3),
                }
                if operand & 0b_111 == 6 {
                    match operand {
                        0x40..=0x7F => self.delay(1),
                        _ => self.delay(2),
                    }
                }
            }
            0xE0 | 0xE2 | 0xEA | 0xF0 | 0xF2 | 0xFA => {
                let addr = match opcode & 0b_1111 {
                    0x0 => 0xFF00 + operand as u16,
                    0x2 => 0xFF00 + self.get_reg(C) as u16,
                    0xA => operand_16,
                    _ => unreachable!(),
                };
                if opcode & 0b_10000 > 0 {
                    let value = self.bus.borrow().get(addr);
                    self.set_reg(A, value);
                } else {
                    self.bus.borrow_mut().set(addr, self.get_reg(A));
                }
            }
            0xE8 | 0xF8 => {
                let sp = self.get_reg_16(SP);
                let result = add_u16_i8(sp, operand as i8);
                self.set_reg_16([SP, HL][opcode as usize - 0xE8 >> 4], result);
                self.set_flag(ZF, false);
                self.set_flag(NF, false);
                self.set_flag(HF, (sp & 0x0F) + (operand as u16 & 0x0F) > 0x0F);
                self.set_flag(CF, (sp & 0xFF) + (operand as u16 & 0xFF) > 0xFF);
            }
            0xF3 => self.ime = false,
            0xFB => self.ei_pending = true,
            0xF9 => self.set_reg_16(SP, self.get_reg_16(HL)),
            _ => self.invalid_opcode(),
        }
        self.delay(INST_BASE_CYCLES[opcode as usize]);
    }

    pub fn int_req(&mut self, int: Interrupt) {
        let mut bus = self.bus.borrow_mut();
        let value = bus.get(RegHw::IF as u16) | 1 << int as u8;
        bus.set(RegHw::IF as u16, value);
        if bus.get(RegHw::IE as u16) & value > 0 {
            self.halting = false;
        }
    }
    fn delay(&mut self, m_cycle: u8) {
        self.next_inst_t_state += m_cycle as u64 * 4;
    }

    fn invalid_opcode(&self) {}

    #[cfg(feature = "cpu-trace")]
    fn trace_pc(&mut self) {
        let bank_pc = if (self.pc as u32) < 0x4000 {
            self.pc as u32
        } else {
            self.bus.borrow().rom_bank as u32 * 0x1000000 + (self.pc as u32)
        };
        if self.trace.insert(bank_pc) {
            log::trace!("{:08X}", bank_pc);
        }
    }

    #[cfg(not(feature = "cpu-trace"))]
    fn trace_pc(&mut self) {}
}

fn get_flag_mask(flag: Flag) -> u8 {
    match flag {
        ZF => 0b_10000000,
        NF => 0b_01000000,
        HF => 0b_00100000,
        CF => 0b_00010000,
    }
}

fn id_to_reg(id: u8) -> Reg {
    match id {
        0 => B,
        1 => C,
        2 => D,
        3 => E,
        4 => H,
        5 => L,
        6 => AddrHL,
        7 => A,
        _ => panic!(),
    }
}
fn id_to_reg_16(id: u8) -> Reg16 {
    match id {
        0 => BC,
        1 => DE,
        2 => HL,
        3 => SP,
        _ => panic!(),
    }
}
fn id_to_alu_op(id: u8) -> fn(&mut Cpu, Reg) {
    match id {
        0 => Cpu::add,
        1 => Cpu::adc,
        2 => Cpu::sub,
        3 => Cpu::sbc,
        4 => Cpu::and,
        5 => Cpu::xor,
        6 => Cpu::or,
        7 => Cpu::cp,
        _ => panic!(),
    }
}

fn add_u16_i8(lhs: u16, rhs: i8) -> u16 {
    (lhs as i16 + rhs as i16) as u16
}
