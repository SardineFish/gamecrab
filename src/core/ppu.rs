use alloc::{vec, vec::Vec};
use core::ptr::NonNull;

use super::{
    bus::{oam::Obj, Bus},
    clock::Clock,
    emu::RegHw,
};

const T_STATES_PER_LINE: u64 = 456;
const LINES_PER_FRAME: u8 = 154;
const SCRN_X: u8 = 160;
const SCRN_Y: u8 = 144;

pub struct Ppu {
    bus: NonNull<Bus>,
    clock: NonNull<Clock>,
    pub framebuffer: Vec<u8>,
    pub current_line: u8,
    pub frame_count: u64,
    next_line_t_state: u64,
    pub irq_vblank: bool,
    pub irq_lcd: bool,
}

impl Ppu {
    pub fn new(bus: NonNull<Bus>, clock: NonNull<Clock>) -> Self {
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

    fn bus(&self) -> &Bus {
        unsafe { self.bus.as_ref() }
    }

    fn bus_mut(&mut self) -> &mut Bus {
        unsafe { self.bus.as_mut() }
    }

    fn clock(&self) -> &Clock {
        unsafe { self.clock.as_ref() }
    }

    /**
     * WARNING: Inaccurate implementation
     */
    pub fn tick(&mut self) -> bool {
        let t_state = self.clock().get_t_state();
        self.tick_at(t_state)
    }

    pub fn tick_at(&mut self, t_state: u64) -> bool {
        if t_state < self.next_line_t_state {
            return false;
        }

        self.next_line_t_state += T_STATES_PER_LINE;
        let lcdc = self.bus().get(RegHw::LCDC as u16);
        let current_line = self.current_line;
        self.bus_mut().set(RegHw::LY as u16, current_line);
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

        true
    }

    fn draw_bg(&mut self) {
        let y = self.current_line;
        let bus = unsafe { self.bus.as_ref() };
        let framebuffer = &mut self.framebuffer;
        let lcdc = bus.get(RegHw::LCDC as u16);
        let alt_tiles = lcdc >> 4 & 1 == 0;
        let bg_offset_x = bus.get(RegHw::SCX as u16);
        let bg_offset_y = bus.get(RegHw::SCY as u16);
        let bg_map = lcdc as u16 >> 3 & 1;
        let bg_palette = decode_palette(bus.get(0xFF47));
        for x in 0..SCRN_X {
            let bg_x = bg_offset_x.wrapping_add(x);
            let bg_y = bg_offset_y.wrapping_add(y);
            let tilemap_x = bg_x / 8;
            let tilemap_y = bg_y / 8;
            let tilemap_idx = tilemap_y as u16 * 32 + tilemap_x as u16;
            let mut tile_id = bus.vram[(0x1800 + bg_map * 0x400 + tilemap_idx) as usize] as u16;
            if alt_tiles && tile_id < 128 {
                tile_id += 256;
            }
            let tile_x = bg_x % 8;
            let tile_y = bg_y % 8;
            let addr = tile_id as usize * 16 + tile_y as usize * 2;
            let lsb = bus.vram[addr] >> 7 - tile_x & 1;
            let msb = bus.vram[addr + 1] >> 7 - tile_x & 1;
            let color = bg_palette[(lsb | msb << 1) as usize];
            framebuffer[y as usize * SCRN_X as usize + x as usize] = color;
        }
    }
    /**
     * Unimplemented: layer priority
     */
    fn draw_obj(&mut self) {
        let y = self.current_line;
        let bus = unsafe { self.bus.as_ref() };
        let framebuffer = &mut self.framebuffer;
        let lcdc = bus.get(RegHw::LCDC as u16);
        let obj_height = if lcdc >> 2 & 1 == 0 { 8 } else { 16 };
        let obj0_palette = decode_palette(bus.get(0xFF48));
        let obj1_palette = decode_palette(bus.get(0xFF49));
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
            let addr = tile_id as usize * 16 + tile_y as usize * 2;
            let (byte0, byte1) = (bus.vram[addr], bus.vram[addr + 1]);
            for i in 0..8 {
                let x = obj_x as i16 - 8 + i;
                if x >= 0 && x < SCRN_X as i16 {
                    let flip_x = attr >> 5 & 1 > 0;
                    let tile_x = if flip_x { 7 - i } else { i };
                    let (lsb, msb) = (byte0 >> 7 - tile_x & 1, byte1 >> 7 - tile_x & 1);
                    let color_id = lsb | msb << 1;
                    if color_id > 0 {
                        let palette = if attr >> 4 & 1 == 0 {
                            obj0_palette
                        } else {
                            obj1_palette
                        };
                        let color = palette[color_id as usize];
                        framebuffer[y as usize * SCRN_X as usize + x as usize] = color;
                    }
                }
            }
        }
    }
}

fn decode_palette(palette_data: u8) -> [u8; 4] {
    [
        palette_data >> 0 & 0b_11,
        palette_data >> 2 & 0b_11,
        palette_data >> 4 & 0b_11,
        palette_data >> 6 & 0b_11,
    ]
}
