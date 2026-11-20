// 运行时绘制的太阳图标（窗口 / 托盘，跟随明暗主题）
const RAYS: f32 = 8.0;

pub fn sun_rgba(size: u32, for_dark: bool) -> (Vec<u8>, u32, u32) {
    let c = if for_dark { (255u8, 255u8, 255u8) } else { (10u8, 11u8, 15u8) };
    sun_rgba_rgb(size, c)
}

pub fn sun_rgba_rgb(size: u32, color: (u8, u8, u8)) -> (Vec<u8>, u32, u32) {
    let n = size as f32;
    let mut px = vec![0u8; (size * size * 4) as usize];

    let mid = n * 0.5;
    let r_ring = n * 0.240;
    let w_ring = n * 0.052;
    let r_in = n * 0.350;
    let r_out = n * 0.465;
    let w_ray = n * 0.046;

    let (cr, cg, cb) = color;

    const SS: i32 = 5;
    let step = std::f32::consts::TAU / RAYS;

    for y in 0..size as i32 {
        for x in 0..size as i32 {
            let mut cov = 0.0f32;
            for sy in 0..SS {
                for sx in 0..SS {
                    let px_ = x as f32 + (sx as f32 + 0.5) / SS as f32 - mid;
                    let py_ = y as f32 + (sy as f32 + 0.5) / SS as f32 - mid;
                    let d = (px_ * px_ + py_ * py_).sqrt();

                    let mut hit = (d - r_ring).abs() <= w_ring;

                    if !hit && d >= r_in - w_ray && d <= r_out + w_ray {
                        let ang = py_.atan2(px_);
                        let (s, co) = (ang / step).round().mul_add(step, 0.0).sin_cos();
                        let along = (px_ * co + py_ * s).clamp(r_in, r_out);
                        let dx = px_ - co * along;
                        let dy = py_ - s * along;
                        hit = (dx * dx + dy * dy).sqrt() <= w_ray;
                    }

                    if hit {
                        cov += 1.0;
                    }
                }
            }

            let a = cov / (SS * SS) as f32;
            if a <= 0.0 {
                continue;
            }
            let i = ((y * size as i32 + x) * 4) as usize;
            px[i] = cr;
            px[i + 1] = cg;
            px[i + 2] = cb;
            px[i + 3] = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }

    (px, size, size)
}

pub fn write_ico(path: &std::path::Path) -> std::io::Result<()> {
    let sizes: [u32; 9] = [16, 20, 24, 32, 40, 48, 64, 128, 256];
    let color = (247u8, 168u8, 42u8);
    let mut frames: Vec<Vec<u8>> = Vec::new();
    for s in sizes {
        let (rgba, w, h) = sun_rgba_rgb(s, color);
        let mask_row = ((w as usize + 31) / 32) * 4;
        let mut f = Vec::with_capacity(40 + (w * h * 4) as usize + mask_row * h as usize);
        f.extend_from_slice(&40u32.to_le_bytes());
        f.extend_from_slice(&(w as i32).to_le_bytes());
        f.extend_from_slice(&((h as i32) * 2).to_le_bytes());
        f.extend_from_slice(&1u16.to_le_bytes());
        f.extend_from_slice(&32u16.to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&(w * h * 4).to_le_bytes());
        f.extend_from_slice(&[0u8; 16]);
        for y in (0..h).rev() {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                f.push(rgba[i + 2]);
                f.push(rgba[i + 1]);
                f.push(rgba[i]);
                f.push(rgba[i + 3]);
            }
        }
        f.extend(std::iter::repeat(0u8).take(mask_row * h as usize));
        frames.push(f);
    }
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(frames.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * frames.len();
    for (i, f) in frames.iter().enumerate() {
        let s = sizes[i];
        out.push(if s >= 256 { 0 } else { s as u8 });
        out.push(if s >= 256 { 0 } else { s as u8 });
        out.push(0);
        out.push(0);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(f.len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += f.len();
    }
    for f in frames {
        out.extend_from_slice(&f);
    }
    std::fs::write(path, out)
}

pub fn tray_icon(for_dark: bool) -> Option<tray_icon::Icon> {
    let (rgba, w, h) = sun_rgba(48, for_dark);
    tray_icon::Icon::from_rgba(rgba, w, h).ok()
}

pub fn hicon(for_dark: bool, size: i32) -> windows::Win32::UI::WindowsAndMessaging::HICON {
    use std::ffi::c_void;
    use windows::core::BOOL;
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
        DeleteObject,
    };
    use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, HICON, ICONINFO};

    let (rgba, w, h) = sun_rgba(size as u32, for_dark);
    unsafe {
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w as i32,
            biHeight: -(h as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0,
            ..Default::default()
        };
        let _ = BI_RGB;
        let mut bits: *mut c_void = std::ptr::null_mut();
        let Ok(color) = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) else {
            return HICON::default();
        };
        if !bits.is_null() {
            let dst = std::slice::from_raw_parts_mut(bits as *mut u8, (w * h * 4) as usize);
            for i in 0..(w * h) as usize {
                dst[i * 4] = rgba[i * 4 + 2];
                dst[i * 4 + 1] = rgba[i * 4 + 1];
                dst[i * 4 + 2] = rgba[i * 4];
                dst[i * 4 + 3] = rgba[i * 4 + 3];
            }
        }
        let mask_row = ((w as usize + 31) / 32) * 4;
        let mask_bits = vec![0u8; mask_row * h as usize];
        let mask = CreateBitmap(
            w as i32,
            h as i32,
            1,
            1,
            Some(mask_bits.as_ptr() as *const c_void),
        );
        let info = ICONINFO {
            fIcon: BOOL(1),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = CreateIconIndirect(&info).unwrap_or_default();
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
        icon
    }
}
