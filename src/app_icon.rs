// App icon by AUMID through the Shell ("shell:AppsFolder\<AUMID>", what the Start menu uses).
// Works for unpackaged Win32 apps, where AppDisplayInfo::GetLogo returns null.
// ponytail: tiny PNG encoder (stored deflate) so the result is a plain data URL, no new crates.
use windows::core::HSTRING;
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
};
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
};

use crate::media::to_base64;

const MAX_SIDE: i32 = 256;

pub fn icon_data_url(aumid: &str, size: i32) -> Option<String> {
    if aumid.is_empty() {
        return None;
    }
    let (w, h, rgba) = shell_icon_rgba(aumid, size)?;
    Some(format!("data:image/png;base64,{}", to_base64(&encode_png(w, h, &rgba))))
}

fn shell_icon_rgba(aumid: &str, size: i32) -> Option<(u32, u32, Vec<u8>)> {
    unsafe {
        let path = HSTRING::from(format!("shell:AppsFolder\\{aumid}"));
        let factory: IShellItemImageFactory = SHCreateItemFromParsingName(&path, None).ok()?;
        let hbmp = factory
            .GetImage(SIZE { cx: size, cy: size }, SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK)
            .ok()?;
        let obj = HGDIOBJ(hbmp.0);

        let mut bmp = BITMAP::default();
        let got = GetObjectW(
            obj,
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bmp as *mut BITMAP as *mut core::ffi::c_void),
        );
        let (w, h) = (bmp.bmWidth, bmp.bmHeight.abs());
        if got == 0 || w <= 0 || h <= 0 || w > MAX_SIDE || h > MAX_SIDE {
            let _ = DeleteObject(obj);
            return None;
        }

        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down rows
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bgra = vec![0u8; (w * h * 4) as usize];
        let hdc = GetDC(None);
        let lines = GetDIBits(
            hdc,
            hbmp,
            0,
            h as u32,
            Some(bgra.as_mut_ptr() as *mut core::ffi::c_void),
            &mut info,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, hdc);
        let _ = DeleteObject(obj);
        if lines == 0 {
            return None;
        }
        Some((w as u32, h as u32, bgra_premul_to_rgba(&bgra)))
    }
}

// Shell bitmaps are premultiplied BGRA; PNG wants straight RGBA. No alpha at all = opaque.
fn bgra_premul_to_rgba(bgra: &[u8]) -> Vec<u8> {
    let (pixels, _) = bgra.as_chunks::<4>();
    let has_alpha = pixels.iter().any(|p| p[3] != 0);
    let mut out = Vec::with_capacity(bgra.len());
    for &[b, g, r, a] in pixels {
        let (b, g, r, a) = (b as u32, g as u32, r as u32, a as u32);
        if !has_alpha {
            out.extend_from_slice(&[r as u8, g as u8, b as u8, 255]);
        } else if a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            let un = |c: u32| ((c * 255 + a / 2) / a).min(255) as u8;
            out.extend_from_slice(&[un(r), un(g), un(b), a as u8]);
        }
    }
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut body = Vec::with_capacity(4 + data.len());
    body.extend_from_slice(kind);
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
}

// 8-bit RGBA PNG; zlib stream with "stored" deflate blocks (icons are tiny, no compression needed)
fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((w * 4 + 1) as usize * h as usize);
    for row in rgba.chunks_exact((w * 4) as usize) {
        raw.push(0); // filter: none
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65_535).peekable();
    while let Some(block) = blocks.next() {
        z.push(if blocks.peek().is_none() { 1 } else { 0 });
        let len = block.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(block);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit, RGBA, deflate, no filter, no interlace

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"IDAT", &z);
    chunk(&mut png, b"IEND", &[]);
    png
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

    #[test]
    fn png_has_valid_crc_for_iend() {
        // IEND's CRC is a fixed, well-known value
        let png = encode_png(1, 1, &[255, 0, 0, 255]);
        assert_eq!(&png[png.len() - 4..], &[0xAE, 0x42, 0x60, 0x82]);
    }

    #[test]
    fn premultiplied_alpha_is_undone() {
        // 50% red premultiplied (BGRA) -> full red at alpha 128
        assert_eq!(bgra_premul_to_rgba(&[0, 0, 128, 128]), vec![255, 0, 0, 128]);
    }

    // Manual probe against the real Shell: cargo test --offline powershell_icon -- --nocapture
    #[test]
    fn powershell_icon() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        let url = icon_data_url(
            "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe",
            32,
        );
        println!("icon: {:?}", url.as_ref().map(|u| u.len()));
        assert!(url.is_some(), "Shell icon for PowerShell should resolve");
        // Optional: WMO_ICON_OUT=<file.png> dumps it to look at
        if let Ok(out) = std::env::var("WMO_ICON_OUT") {
            let aumid = "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe";
            if let Some((w, h, rgba)) = shell_icon_rgba(aumid, 64) {
                let _ = std::fs::write(out, encode_png(w, h, &rgba));
            }
        }
    }
}
