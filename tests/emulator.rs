use std::{
    fs::{create_dir_all, File},
    io::{self, Write},
    path::Path,
    thread,
};

use gamecrab::core::emu::Emu;
use memmap2::Mmap;

const MAX_TICKS: usize = 25_000_000;
const FRAME_TO_CAPTURE: u64 = 300;
const GAMEPLAY_INITIAL_WAIT_FRAMES: u64 = 60;
const GAMEPLAY_BUTTON_HOLD_FRAMES: u64 = 10;
const GAMEPLAY_BUTTON_GAP_FRAMES: u64 = 60;
const GAMEPLAY_FINAL_WAIT_FRAMES: u64 = 300;
const SCREEN_WIDTH: usize = 160;
const SCREEN_HEIGHT: usize = 144;
const EXPECTED_GBLINEZ_FRAME_HASH: u64 = 5942243365668119245;
const EXPECTED_GBLINEZ_GAMEPLAY_HASH: u64 = 3041149544561916889;
const FIXTURE_DIR: &str = "tests/fixtures/blargg/cpu_instrs/individual";
const PALETTE: &[(u8, u8, u8)] = &[
    (255, 255, 255),
    (170, 170, 170),
    (85, 85, 85),
    (0, 0, 0),
];
const BLARGG_ROMS: &[(&str, &str)] = &[
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

#[derive(Clone, Copy)]
enum Button {
    Start,
    A,
    Down,
    Right,
}

#[test]
fn gblinez_frame_300_matches_expected_output() {
    create_dir_all("target/test-output").unwrap();
    std::env::set_var("GAMECRAB_LOG_PATH", "target/test-output/gblinez-log.txt");

    let file = File::open("tests/game/gblinez.gb").unwrap();
    let rom = unsafe { Mmap::map(&file).unwrap() };
    let mut emu = Emu::new(rom, None);

    while emu.ppu.frame_count < FRAME_TO_CAPTURE {
        emu.tick();
    }

    let rgb = framebuffer_to_rgb(&emu.ppu.framebuffer);
    let hash = fnv1a64(&rgb);
    let output_path = Path::new("target/test-output/gblinez-frame-300.bmp");
    write_bmp(output_path, SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32, &rgb).unwrap();

    assert_eq!(
        hash,
        EXPECTED_GBLINEZ_FRAME_HASH,
        "gblinez frame {FRAME_TO_CAPTURE} hash changed; wrote {}",
        output_path.display(),
    );
}

#[test]
fn gblinez_scripted_gameplay_matches_expected_output() {
    create_dir_all("target/test-output").unwrap();
    std::env::set_var(
        "GAMECRAB_LOG_PATH",
        "target/test-output/gblinez-gameplay-log.txt",
    );

    let file = File::open("tests/game/gblinez.gb").unwrap();
    let rom = unsafe { Mmap::map(&file).unwrap() };
    let mut emu = Emu::new(rom, None);

    advance_frames(&mut emu, GAMEPLAY_INITIAL_WAIT_FRAMES);
    for button in [
        Button::Start,
        Button::Start,
        Button::Start,
        Button::Down,
        Button::Down,
        Button::A,
        Button::Right,
        Button::A,
    ] {
        press_button(&mut emu, button, true);
        advance_frames(&mut emu, GAMEPLAY_BUTTON_HOLD_FRAMES);
        press_button(&mut emu, button, false);
        advance_frames(&mut emu, GAMEPLAY_BUTTON_GAP_FRAMES);
    }
    advance_frames(&mut emu, GAMEPLAY_FINAL_WAIT_FRAMES);

    let rgb = framebuffer_to_rgb(&emu.ppu.framebuffer);
    let hash = fnv1a64(&rgb);
    let output_path = Path::new("target/test-output/gblinez-gameplay-script.bmp");
    write_bmp(output_path, SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32, &rgb).unwrap();

    assert_eq!(
        hash,
        EXPECTED_GBLINEZ_GAMEPLAY_HASH,
        "gblinez scripted gameplay hash changed; wrote {}",
        output_path.display(),
    );
}

#[test]
fn blargg_cpu_instrs_run_in_parallel() {
    let results = BLARGG_ROMS
        .iter()
        .map(|&(name, sha256)| thread::spawn(move || run_blargg_rom(name, sha256)))
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();

    for (name, output) in results {
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

fn run_blargg_rom(name: &str, expected_sha256: &str) -> (String, String) {
    let path = Path::new(FIXTURE_DIR).join(name);
    assert_fixture_hash(&path, expected_sha256);

    let file = File::open(&path).unwrap();
    let rom = unsafe { Mmap::map(&file).unwrap() };
    let mut emu = Emu::new(rom, None);

    for _ in 0..MAX_TICKS {
        emu.tick();
        let output = serial_output(&emu);
        if output.contains("Passed") || output.contains("Failed") {
            return (name.to_string(), output);
        }
    }

    (name.to_string(), serial_output(&emu))
}

fn advance_frames(emu: &mut Emu, frames: u64) {
    let target_frame = emu.ppu.frame_count + frames;
    while emu.ppu.frame_count < target_frame {
        emu.tick();
    }
}

fn press_button(emu: &mut Emu, button: Button, pressed: bool) {
    let mut bus = emu.bus.borrow_mut();
    match button {
        Button::Start => bus.gamepad.start = pressed,
        Button::A => bus.gamepad.a = pressed,
        Button::Down => bus.gamepad.down = pressed,
        Button::Right => bus.gamepad.right = pressed,
    }
}

fn framebuffer_to_rgb(framebuffer: &[u8]) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(SCREEN_WIDTH * SCREEN_HEIGHT * 3);
    for &color_id in framebuffer {
        let (r, g, b) = PALETTE[color_id as usize];
        rgb.extend_from_slice(&[r, g, b]);
    }
    rgb
}

fn write_bmp(path: &Path, width: u32, height: u32, rgb: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }

    let row_bytes = width as usize * 3;
    let padding = (4 - row_bytes % 4) % 4;
    let pixel_data_size = (row_bytes + padding) * height as usize;
    let file_size = 14 + 40 + pixel_data_size;
    let mut file = File::create(path)?;

    file.write_all(b"BM")?;
    file.write_all(&(file_size as u32).to_le_bytes())?;
    file.write_all(&[0; 4])?;
    file.write_all(&(54u32).to_le_bytes())?;

    file.write_all(&(40u32).to_le_bytes())?;
    file.write_all(&(width as i32).to_le_bytes())?;
    file.write_all(&(height as i32).to_le_bytes())?;
    file.write_all(&(1u16).to_le_bytes())?;
    file.write_all(&(24u16).to_le_bytes())?;
    file.write_all(&(0u32).to_le_bytes())?;
    file.write_all(&(pixel_data_size as u32).to_le_bytes())?;
    file.write_all(&(2835i32).to_le_bytes())?;
    file.write_all(&(2835i32).to_le_bytes())?;
    file.write_all(&(0u32).to_le_bytes())?;
    file.write_all(&(0u32).to_le_bytes())?;

    for y in (0..height as usize).rev() {
        let row = &rgb[y * row_bytes..(y + 1) * row_bytes];
        for pixel in row.chunks_exact(3) {
            file.write_all(&[pixel[2], pixel[1], pixel[0]])?;
        }
        file.write_all(&[0; 3][..padding])?;
    }

    Ok(())
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
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
