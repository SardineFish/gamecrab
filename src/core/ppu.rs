use alloc::{vec, vec::Vec};
use core::ptr::NonNull;

use super::{
    bus::{oam::Obj, Bus},
    clock::Clock,
    emu::RegHw,
};

const T_STATES_PER_LINE: u64 = 456;
const LINES_PER_FRAME: u8 = 154;
pub const SCRN_X: usize = 160;
pub const SCRN_Y: usize = 144;
const FRAME_PIXELS: usize = SCRN_X * SCRN_Y;
#[cfg(feature = "packed-framebuffer")]
const PIXELS_PER_BYTE: usize = 4;
#[cfg(feature = "packed-framebuffer")]
const PIXEL_MASK: u8 = 0x03;
#[cfg(feature = "packed-framebuffer")]
const FRAMEBUFFER_BYTES: usize = FRAME_PIXELS / PIXELS_PER_BYTE;

pub struct FrameBuffer {
    data: Vec<u8>,
}

impl FrameBuffer {
    pub fn new() -> Self {
        Self {
            #[cfg(not(feature = "packed-framebuffer"))]
            data: vec![0; FRAME_PIXELS],
            #[cfg(feature = "packed-framebuffer")]
            data: vec![0; FRAMEBUFFER_BYTES],
        }
    }

    pub fn pixel_count(&self) -> usize {
        FRAME_PIXELS
    }

    pub fn byte_len(&self) -> usize {
        self.data.len()
    }

    #[cfg(not(feature = "packed-framebuffer"))]
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    #[cfg(not(feature = "packed-framebuffer"))]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn get_pixel_index(&self, index: usize) -> u8 {
        #[cfg(not(feature = "packed-framebuffer"))]
        {
            self.data[index]
        }
        #[cfg(feature = "packed-framebuffer")]
        {
        let (byte_index, shift) = packed_pixel_location(index);
        self.data[byte_index] >> shift & PIXEL_MASK
        }
    }

    pub fn set_pixel_index(&mut self, index: usize, color: u8) {
        debug_assert!(color < 4);
        #[cfg(not(feature = "packed-framebuffer"))]
        {
            self.data[index] = color;
        }
        #[cfg(feature = "packed-framebuffer")]
        {
        let (byte_index, shift) = packed_pixel_location(index);
        let mask = PIXEL_MASK << shift;
        self.data[byte_index] = (self.data[byte_index] & !mask) | ((color & PIXEL_MASK) << shift);
        }
    }

    pub fn get_pixel(&self, x: usize, y: usize) -> u8 {
        self.get_pixel_index(pixel_index(x, y))
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, color: u8) {
        self.set_pixel_index(pixel_index(x, y), color);
    }

    pub fn iter_pixels(&self) -> PixelIter<'_> {
        #[cfg(not(feature = "packed-framebuffer"))]
        {
            self.data.iter().copied()
        }
        #[cfg(feature = "packed-framebuffer")]
        {
        PixelIter {
            framebuffer: self,
            next_index: 0,
            end_index: FRAME_PIXELS,
        }
        }
    }

    pub fn iter_row(&self, y: usize) -> PixelIter<'_> {
        assert!(y < SCRN_Y);
        #[cfg(not(feature = "packed-framebuffer"))]
        {
            self.data[y * SCRN_X..(y + 1) * SCRN_X].iter().copied()
        }
        #[cfg(feature = "packed-framebuffer")]
        {
        let start = y * SCRN_X;
        PixelIter {
            framebuffer: self,
            next_index: start,
            end_index: start + SCRN_X,
        }
        }
    }

    #[cfg(feature = "packed-framebuffer")]
    pub fn packed_bytes(&self) -> &[u8] {
        &self.data
    }
}

impl Default for FrameBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(feature = "packed-framebuffer"))]
impl core::ops::Deref for FrameBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

#[cfg(not(feature = "packed-framebuffer"))]
impl core::ops::DerefMut for FrameBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

#[cfg(not(feature = "packed-framebuffer"))]
pub type PixelIter<'a> = core::iter::Copied<core::slice::Iter<'a, u8>>;

#[cfg(feature = "packed-framebuffer")]
pub struct PixelIter<'a> {
    framebuffer: &'a FrameBuffer,
    next_index: usize,
    end_index: usize,
}

#[cfg(feature = "packed-framebuffer")]
impl Iterator for PixelIter<'_> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_index >= self.end_index {
            return None;
        }

        let color = self.framebuffer.get_pixel_index(self.next_index);
        self.next_index += 1;
        Some(color)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.end_index - self.next_index;
        (remaining, Some(remaining))
    }
}

#[cfg(feature = "packed-framebuffer")]
impl ExactSizeIterator for PixelIter<'_> {}

#[cfg(feature = "packed-framebuffer")]
fn packed_pixel_location(index: usize) -> (usize, u8) {
    assert!(index < FRAME_PIXELS);
    let byte_index = index / PIXELS_PER_BYTE;
    let shift = ((index % PIXELS_PER_BYTE) * 2) as u8;
    (byte_index, shift)
}

fn pixel_index(x: usize, y: usize) -> usize {
    assert!(x < SCRN_X);
    assert!(y < SCRN_Y);
    y * SCRN_X + x
}

fn new_framebuffer() -> FrameBuffer {
    FrameBuffer::new()
}

fn set_framebuffer_pixel(framebuffer: &mut FrameBuffer, x: usize, y: usize, color: u8) {
    framebuffer.set_pixel(x, y, color);
}

pub fn framebuffer_pixels(framebuffer: &FrameBuffer) -> PixelIter<'_> {
    framebuffer.iter_pixels()
}

pub fn framebuffer_row(framebuffer: &FrameBuffer, y: usize) -> PixelIter<'_> {
    framebuffer.iter_row(y)
}

pub struct Ppu {
    bus: NonNull<Bus>,
    clock: NonNull<Clock>,
    pub framebuffer: FrameBuffer,
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
            framebuffer: new_framebuffer(),
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

    pub fn next_line_t_state(&self) -> u64 {
        self.next_line_t_state
    }

    pub fn tick_at(&mut self, t_state: u64) -> bool {
        if t_state < self.next_line_t_state {
            return false;
        }

        self.next_line_t_state += T_STATES_PER_LINE;
        let lcdc = self.bus().get(RegHw::LCDC as u16);
        let current_line = self.current_line;
        self.bus_mut().set(RegHw::LY as u16, current_line);
        if usize::from(self.current_line) < SCRN_Y {
            if lcdc >> 7 > 0 {
                if lcdc >> 0 & 1 > 0 {
                    self.draw_bg();
                }
                if lcdc >> 1 & 1 > 0 {
                    self.draw_obj();
                }
            }
        } else if usize::from(self.current_line) == SCRN_Y {
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
        for x in 0..SCRN_X as u8 {
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
            set_framebuffer_pixel(framebuffer, x as usize, y as usize, color);
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
                        set_framebuffer_pixel(framebuffer, x as usize, y as usize, color);
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

#[cfg(test)]
mod tests {
    use super::FrameBuffer;
    use super::SCRN_X;
    #[cfg(not(feature = "packed-framebuffer"))]
    use super::{framebuffer_pixels, framebuffer_row};

    #[test]
    #[cfg(not(feature = "packed-framebuffer"))]
    fn default_framebuffer_keeps_u8_slice_access() {
        let mut framebuffer = FrameBuffer::new();

        framebuffer[0] = 1;
        framebuffer[SCRN_X] = 2;

        assert_eq!(framebuffer.pixel_count(), 160 * 144);
        assert_eq!(framebuffer.byte_len(), 160 * 144);
        assert_eq!(framebuffer.len(), 160 * 144);
        assert_eq!((&framebuffer[..])[0], 1);
        assert_eq!(framebuffer_pixels(&framebuffer).take(SCRN_X + 1).last(), Some(2));
        assert_eq!(framebuffer_row(&framebuffer, 1).next(), Some(2));
    }

    #[test]
    #[cfg(feature = "packed-framebuffer")]
    fn framebuffer_uses_two_bits_per_pixel() {
        let framebuffer = FrameBuffer::new();

        assert_eq!(framebuffer.pixel_count(), 160 * 144);
        assert_eq!(framebuffer.byte_len(), 5_760);
        assert_eq!(framebuffer.packed_bytes().len(), 5_760);
    }

    #[test]
    #[cfg(feature = "packed-framebuffer")]
    fn set_and_get_pixels_across_byte_boundaries() {
        let mut framebuffer = FrameBuffer::new();

        framebuffer.set_pixel_index(0, 1);
        framebuffer.set_pixel_index(3, 2);
        framebuffer.set_pixel_index(4, 3);
        framebuffer.set_pixel_index(7, 1);

        assert_eq!(framebuffer.get_pixel_index(0), 1);
        assert_eq!(framebuffer.get_pixel_index(1), 0);
        assert_eq!(framebuffer.get_pixel_index(3), 2);
        assert_eq!(framebuffer.get_pixel_index(4), 3);
        assert_eq!(framebuffer.get_pixel_index(7), 1);
    }

    #[test]
    #[cfg(feature = "packed-framebuffer")]
    fn pixels_are_packed_into_two_bit_fields() {
        let mut framebuffer = FrameBuffer::new();

        framebuffer.set_pixel_index(0, 1);
        framebuffer.set_pixel_index(1, 2);
        framebuffer.set_pixel_index(2, 3);
        framebuffer.set_pixel_index(3, 0);

        assert_eq!(framebuffer.packed_bytes()[0], 0b00_11_10_01);
    }

    #[test]
    #[cfg(feature = "packed-framebuffer")]
    fn iterators_return_pixels_in_row_major_order() {
        let mut framebuffer = FrameBuffer::new();

        framebuffer.set_pixel(0, 0, 1);
        framebuffer.set_pixel(1, 0, 2);
        framebuffer.set_pixel(0, 1, 3);

        let pixels = framebuffer.iter_pixels().take(SCRN_X + 1).collect::<Vec<_>>();
        assert_eq!(pixels[0], 1);
        assert_eq!(pixels[1], 2);
        assert_eq!(pixels[SCRN_X], 3);

        let row = framebuffer.iter_row(1).take(2).collect::<Vec<_>>();
        assert_eq!(row, vec![3, 0]);
    }
}
