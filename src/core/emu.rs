use std::{cell::RefCell, rc::Rc};

use memmap2::{Mmap, MmapMut};

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

#[cfg(test)]
mod tests {
    use std::{fs::File, path::Path};

    use memmap2::Mmap;

    use super::Emu;

    const MAX_TICKS: usize = 25_000_000;
    const FIXTURE_DIR: &str = "tests/fixtures/blargg/cpu_instrs/individual";
    const ROMS: &[(&str, &str)] = &[
        (
            "01-special.gb",
            "fe61349cbaee10cc384b50f356e541c90d1bc380185716706b5d8c465a03cf89",
        ),
        (
            "02-interrupts.gb",
            "fb90b0d2b9501910c49709abda1d8e70f757dc12020ebf8409a7779bbfd12229",
        ),
        (
            "03-op sp,hl.gb",
            "ca553e606d9b9c86fbd318f1b916c6f0b9df0cf1774825d4361a3fdff2e5a136",
        ),
        (
            "04-op r,imm.gb",
            "7686aa7a39ef3d2520ec1037371b5f94dc283fbbfd0f5051d1f64d987bdd6671",
        ),
        (
            "05-op rp.gb",
            "d504adfa0a4c4793436a154f14492f044d38b3c6db9efc44138f3c9ad138b775",
        ),
        (
            "06-ld r,r.gb",
            "17ada54b0b9c1a33cd5429fce5b765e42392189ca36da96312222ffe309e7ed1",
        ),
        (
            "07-jr,jp,call,ret,rst.gb",
            "ab31d3daaaa3a98bdbd9395b64f48c1bdaa889aba5b19dd5aaff4ec2a7d228a3",
        ),
        (
            "08-misc instrs.gb",
            "974a71fe4c67f70f5cc6e98d4dc8c096057ff8a028b7bfa9f7a4330038cf8b7e",
        ),
        (
            "09-op r,r.gb",
            "b28e1be5cd95f22bd1ecacdd33c6f03e607d68870e31a47b15a0229033d5ba2a",
        ),
        (
            "10-bit ops.gb",
            "7f5b8e488c6988b5aaba8c2a74529b7c180c55a58449d5ee89d606a07c53514a",
        ),
        (
            "11-op a,(hl).gb",
            "0ec0cf9fda3f00becaefa476df6fb526c434abd9d4a4beac237c2c2692dac5d3",
        ),
    ];

    #[test]
    #[ignore = "Manual Blargg CPU ROM harness; run with `cargo test blargg_cpu_instrs -- --ignored --nocapture`."]
    fn blargg_cpu_instrs() {
        for &(name, sha256) in ROMS {
            let output = run_blargg_rom(name, sha256);
            println!("{name}: {output}");
            assert!(
                output.contains("Passed"),
                "{name} did not pass before timeout. Serial output:\n{output}",
            );
            assert!(
                !output.contains("Failed"),
                "{name} reported failure. Serial output:\n{output}",
            );
        }
    }

    fn run_blargg_rom(name: &str, expected_sha256: &str) -> String {
        let path = Path::new(FIXTURE_DIR).join(name);
        assert_fixture_hash(&path, expected_sha256);

        let file = File::open(&path).unwrap();
        let rom = unsafe { Mmap::map(&file).unwrap() };
        let mut emu = Emu::new(rom, None);

        for _ in 0..MAX_TICKS {
            emu.tick();
            let output = serial_output(&emu);
            if output.contains("Passed") || output.contains("Failed") {
                return output;
            }
        }

        serial_output(&emu)
    }

    fn serial_output(emu: &Emu) -> String {
        String::from_utf8_lossy(&emu.bus.borrow().serial_output).into_owned()
    }

    fn assert_fixture_hash(path: &Path, expected_sha256: &str) {
        let bytes = std::fs::read(path).unwrap();
        let actual = sha256_hex(&bytes);
        assert_eq!(
            actual,
            expected_sha256,
            "fixture hash mismatch for {}",
            path.display()
        );
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut h = [
            0x6a09e667u32,
            0xbb67ae85,
            0x3c6ef372,
            0xa54ff53a,
            0x510e527f,
            0x9b05688c,
            0x1f83d9ab,
            0x5be0cd19,
        ];
        let bit_len = (bytes.len() as u64) * 8;
        let mut data = bytes.to_vec();
        data.push(0x80);
        while data.len() % 64 != 56 {
            data.push(0);
        }
        data.extend_from_slice(&bit_len.to_be_bytes());

        for chunk in data.chunks_exact(64) {
            let mut w = [0u32; 64];
            for i in 0..16 {
                w[i] = u32::from_be_bytes([
                    chunk[i * 4],
                    chunk[i * 4 + 1],
                    chunk[i * 4 + 2],
                    chunk[i * 4 + 3],
                ]);
            }
            for i in 16..64 {
                let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
                let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
                w[i] = w[i - 16]
                    .wrapping_add(s0)
                    .wrapping_add(w[i - 7])
                    .wrapping_add(s1);
            }

            let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
                (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
            for i in 0..64 {
                let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
                let ch = (e & f) ^ ((!e) & g);
                let temp1 = hh
                    .wrapping_add(s1)
                    .wrapping_add(ch)
                    .wrapping_add(K[i])
                    .wrapping_add(w[i]);
                let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
                let maj = (a & b) ^ (a & c) ^ (b & c);
                let temp2 = s0.wrapping_add(maj);

                hh = g;
                g = f;
                f = e;
                e = d.wrapping_add(temp1);
                d = c;
                c = b;
                b = a;
                a = temp1.wrapping_add(temp2);
            }

            h[0] = h[0].wrapping_add(a);
            h[1] = h[1].wrapping_add(b);
            h[2] = h[2].wrapping_add(c);
            h[3] = h[3].wrapping_add(d);
            h[4] = h[4].wrapping_add(e);
            h[5] = h[5].wrapping_add(f);
            h[6] = h[6].wrapping_add(g);
            h[7] = h[7].wrapping_add(hh);
        }

        let mut hex = String::new();
        for word in h {
            hex.push_str(&format!("{word:08x}"));
        }
        hex
    }
}

pub struct Emu {
    pub bus: Rc<RefCell<Bus>>,
    pub clock: Rc<RefCell<Clock>>,
    pub cpu: Cpu,
    pub ppu: Ppu,
}

impl Emu {
    pub fn new(rom: Mmap, sram: Option<MmapMut>) -> Self {
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
