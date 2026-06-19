use alloc::{rc::Rc, vec, vec::Vec};
use core::cell::RefCell;

use super::{
    bus::{oam::Obj, Bus},
    clock::Clock,
    emu::RegHw,
};

const T_STATES_PER_LINE: u64 = 456;
const LINES_PER_FRAME: u8 = 154;
const SCRN_X: u8 = 160;
const SCRN_Y: u8 = 144;

enum Palette {
    BG,
    OBJ0,
    OBJ1,
}

pub struct Ppu {
    bus: Rc<RefCell<Bus>>,
    clock: Rc<RefCell<Clock>>,
    pub framebuffer: Vec<u8>,
    pub current_line: u8,
    pub frame_count: u64,
    next_line_t_state: u64,
    pub irq_vblank: bool,
    pub irq_lcd: bool,
}

impl Ppu {
    pub fn new(bus: Rc<RefCell<Bus>>, clock: Rc<RefCell<Clock>>) -> Self {
        Self {
            bus,
            clock,
            framebuffer: vec![0; SCRN_X as usize * SCRN_Y as usize],
            current_line: 0,
            frame_count: 0,
            next_line_t_state: 0,
            irq_vblank: false,
            irq_lcd: false,
        }
    }

    fn get_palette(&self, palette_type: Palette) -> [u8; 4] {
        let addr = 0xFF47 + palette_type as u16;
        let palette_data = self.bus.borrow().get(addr);
        [
            palette_data >> 0 & 0b_11,
            palette_data >> 2 & 0b_11,
            palette_data >> 4 & 0b_11,
            palette_data >> 6 & 0b_11,
        ]
    }
    fn get_bg_offset(&self) -> (u8, u8) {
        (self.bus.borrow().get(0xFF43), self.bus.borrow().get(0xFF42))
    }

    /**
     * WARNING: Inaccurate implementation
     */
    pub fn tick(&mut self) {
        let lcdc = self.bus.borrow().get(RegHw::LCDC as u16);
        if self.clock.borrow().get_t_state() < self.next_line_t_state {
            return;
        }
        self.next_line_t_state += T_STATES_PER_LINE;
        self.bus
            .borrow_mut()
            .set(RegHw::LY as u16, self.current_line);
        if self.current_line < SCRN_Y {
            if lcdc >> 7 > 0 {
                if lcdc >> 0 & 1 > 0 {
                    self.draw_bg();
                }
                if lcdc >> 1 & 1 > 0 {
                    self.draw_obj();
                }
            }
        } else if self.current_line == SCRN_Y {
            self.irq_vblank = true;
        }
        self.current_line += 1;
        if self.current_line >= LINES_PER_FRAME {
            self.current_line = 0;
            self.frame_count += 1;
        }
    }

    fn draw_bg(&mut self) {
        let y = self.current_line;
        let bus = self.bus.borrow();
        let lcdc = bus.get(RegHw::LCDC as u16);
        let alt_tiles = lcdc >> 4 & 1 == 0;
        let (bg_offset_x, bg_offset_y) = self.get_bg_offset();
        let bg_map = lcdc as u16 >> 3 & 1;
        let bg_palette = self.get_palette(Palette::BG);
        for x in 0..SCRN_X {
            let tilemap_x = (bg_offset_x + x) / 8;
            let tilemap_y = (bg_offset_y + y) / 8;
            let tilemap_idx = tilemap_y as u16 * 32 + tilemap_x as u16;
            let mut tile_id = bus.get(0x9800 + bg_map * 0x400 + tilemap_idx) as u16;
            if alt_tiles && tile_id < 128 {
                tile_id += 256;
            }
            let tile_x = (bg_offset_x + x) % 8;
            let tile_y = (bg_offset_y + y) % 8;
            let addr = 0x8000 + tile_id as u16 * 16 + tile_y as u16 * 2;
            let lsb = bus.get(addr + 0) >> 7 - tile_x & 1;
            let msb = bus.get(addr + 1) >> 7 - tile_x & 1;
            let color = bg_palette[(lsb | msb << 1) as usize];
            self.framebuffer[y as usize * SCRN_X as usize + x as usize] = color;
        }
    }
    /**
     * Unimplemented: layer priority
     */
    fn draw_obj(&mut self) {
        let y = self.current_line;
        let bus = self.bus.borrow();
        let lcdc = bus.get(RegHw::LCDC as u16);
        let obj_height = if lcdc >> 2 & 1 == 0 { 8 } else { 16 };
        for &Obj {
            x: obj_x,
            y: obj_y,
            tile_id,
            attr,
        } in bus
            .oam
            .objects
            .iter()
            .filter(|&obj| obj.y <= y + 16 && obj.y > y + 16 - obj_height)
            .take(10)
        {
            let mut tile_y = y + 16 - obj_y;
            let flip_y = attr >> 6 & 1 > 0;
            if flip_y {
                tile_y = obj_height - 1 - tile_y;
            }
            let addr = 0x8000 + tile_id as u16 * 16 + tile_y as u16 * 2;
            let (byte0, byte1) = (bus.get(addr + 0), bus.get(addr + 1));
            for i in 0..8 {
                let x = obj_x as i16 - 8 + i;
                if x >= 0 && x < SCRN_X as i16 {
                    let flip_x = attr >> 5 & 1 > 0;
                    let tile_x = if flip_x { 7 - i } else { i };
                    let (lsb, msb) = (byte0 >> 7 - tile_x & 1, byte1 >> 7 - tile_x & 1);
                    let color_id = lsb | msb << 1;
                    if color_id > 0 {
                        let palette = if attr >> 4 & 1 == 0 {
                            self.get_palette(Palette::OBJ0)
                        } else {
                            self.get_palette(Palette::OBJ1)
                        };
                        let color = palette[color_id as usize];
                        self.framebuffer[y as usize * SCRN_X as usize + x as usize] = color;
                    }
                }
            }
        }
    }
}
