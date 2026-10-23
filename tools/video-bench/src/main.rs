// Scratch benchmark for the iidxbg video research:
//   upload  - D3D9Ex 1080p dynamic-texture refresh cost (lock / copy / unlock)
//   decode  - Media Foundation Source Reader decode to BGRA at a canvas size
use std::time::Instant;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D9::*;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone, Copy)]
#[repr(C)]
struct Vertex {
    x: f32,
    y: f32,
    z: f32,
    rhw: f32,
    color: u32,
    u: f32,
    v: f32,
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn window() -> HWND {
    unsafe {
        let instance = GetModuleHandleW(None).unwrap();
        let class = w!("iidxbg-video-bench");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassExW(&wc);
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!("iidxbg video bench"),
            WS_POPUP,
            0,
            0,
            64,
            64,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .unwrap()
    }
}

unsafe fn device(hwnd: HWND) -> IDirect3DDevice9Ex {
    unsafe {
        let d3d: IDirect3D9Ex = Direct3DCreate9Ex(D3D_SDK_VERSION).unwrap();
        let mut pp = D3DPRESENT_PARAMETERS {
            BackBufferWidth: 1920,
            BackBufferHeight: 1080,
            BackBufferFormat: D3DFMT_X8R8G8B8,
            BackBufferCount: 1,
            SwapEffect: D3DSWAPEFFECT_DISCARD,
            hDeviceWindow: hwnd,
            Windowed: TRUE,
            PresentationInterval: D3DPRESENT_INTERVAL_IMMEDIATE as u32,
            ..Default::default()
        };
        let mut device = None;
        d3d.CreateDeviceEx(
            0,
            D3DDEVTYPE_HAL,
            hwnd,
            (D3DCREATE_MULTITHREADED | D3DCREATE_HARDWARE_VERTEXPROCESSING) as u32,
            &mut pp,
            std::ptr::null_mut(),
            &mut device,
        )
        .unwrap();
        device.unwrap()
    }
}

unsafe fn create_texture(device: &IDirect3DDevice9Ex, w: u32, h: u32) -> IDirect3DTexture9 {
    unsafe {
        let mut texture = None;
        device
            .CreateTexture(
                w,
                h,
                1,
                D3DUSAGE_DYNAMIC as u32,
                D3DFMT_A8R8G8B8,
                D3DPOOL_DEFAULT,
                &mut texture,
                std::ptr::null_mut(),
            )
            .unwrap();
        texture.unwrap()
    }
}

unsafe fn draw(device: &IDirect3DDevice9Ex, texture: &IDirect3DTexture9) {
    let vertices = [
        Vertex {
            x: -0.5,
            y: -0.5,
            z: 0.0,
            rhw: 1.0,
            color: 0xffffffff,
            u: 0.0,
            v: 0.0,
        },
        Vertex {
            x: -0.5,
            y: 1079.5,
            z: 0.0,
            rhw: 1.0,
            color: 0xffffffff,
            u: 0.0,
            v: 1.0,
        },
        Vertex {
            x: 1919.5,
            y: 1079.5,
            z: 0.0,
            rhw: 1.0,
            color: 0xffffffff,
            u: 1.0,
            v: 1.0,
        },
        Vertex {
            x: 1919.5,
            y: -0.5,
            z: 0.0,
            rhw: 1.0,
            color: 0xffffffff,
            u: 1.0,
            v: 0.0,
        },
    ];
    unsafe {
        let _ = device.BeginScene();
        let _ = device.SetFVF(0x144);
        let _ = device.SetTexture(0, texture);
        let _ = device.DrawPrimitiveUP(
            D3DPT_TRIANGLEFAN,
            2,
            vertices.as_ptr().cast(),
            std::mem::size_of::<Vertex>() as u32,
        );
        let _ = device.EndScene();
        let _ = device.Present(
            std::ptr::null(),
            std::ptr::null(),
            HWND::default(),
            std::ptr::null(),
        );
    }
}

unsafe fn copy_rows(src: *const u8, dst: *mut u8, pitch: usize, width: usize, height: usize) {
    for y in 0..height {
        unsafe {
            std::ptr::copy_nonoverlapping(src.add(y * width), dst.add(y * pitch), width);
        }
    }
}

unsafe fn copy_rows_streaming(
    src: *const u8,
    dst: *mut u8,
    pitch: usize,
    width: usize,
    height: usize,
) {
    use std::arch::x86_64::*;
    for y in 0..height {
        unsafe {
            let mut source = src.add(y * width);
            let mut target = dst.add(y * pitch);
            let mut remaining = width;
            while remaining > 0 && (target as usize) % 16 != 0 {
                *target = *source;
                target = target.add(1);
                source = source.add(1);
                remaining -= 1;
            }
            while remaining >= 64 {
                let a = _mm_loadu_si128(source.cast());
                let b = _mm_loadu_si128(source.add(16).cast());
                let c = _mm_loadu_si128(source.add(32).cast());
                let d = _mm_loadu_si128(source.add(48).cast());
                _mm_stream_si128(target.cast(), a);
                _mm_stream_si128(target.add(16).cast(), b);
                _mm_stream_si128(target.add(32).cast(), c);
                _mm_stream_si128(target.add(48).cast(), d);
                source = source.add(64);
                target = target.add(64);
                remaining -= 64;
            }
            while remaining >= 16 {
                _mm_stream_si128(target.cast(), _mm_loadu_si128(source.cast()));
                source = source.add(16);
                target = target.add(16);
                remaining -= 16;
            }
            while remaining > 0 {
                *target = *source;
                target = target.add(1);
                source = source.add(1);
                remaining -= 1;
            }
        }
    }
    unsafe { std::arch::x86_64::_mm_sfence() };
}

fn bench_upload() {
    let hwnd = window();
    let device = unsafe { device(hwnd) };
    let mut source = vec![0u8; 1920 * 1080 * 4];
    for (i, b) in source.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    println!("texture create:");
    for (w, h) in [(1920u32, 1080u32), (1280, 720), (960, 540), (640, 360)] {
        let mut total = 0f64;
        for _ in 0..20 {
            let start = Instant::now();
            let texture = unsafe { create_texture(&device, w, h) };
            total += start.elapsed().as_secs_f64() * 1e6;
            drop(texture);
        }
        println!("  {w}x{h}: {:.0} us", total / 20.0);
    }
    for (w, h) in [(1920u32, 1080u32), (1280, 720), (960, 540), (640, 360)] {
        let texture = unsafe { create_texture(&device, w, h) };
        let row_bytes = w as usize * 4;
        for (name, streaming, flags) in [
            ("copy", false, D3DLOCK_DISCARD),
            ("stream", true, D3DLOCK_DISCARD),
            ("copy+nosys", false, D3DLOCK_DISCARD | D3DLOCK_NOSYSLOCK),
            ("stream+nosys", true, D3DLOCK_DISCARD | D3DLOCK_NOSYSLOCK),
            ("copy+nowait", false, D3DLOCK_DISCARD | D3DLOCK_DONOTWAIT),
        ] {
            let mut lock_us = 0f64;
            let mut copy_us = 0f64;
            let mut unlock_us = 0f64;
            let mut failures = 0;
            let frames = 120;
            for frame in 0..frames {
                unsafe { draw(&device, &texture) };
                let mut locked = D3DLOCKED_RECT::default();
                let start = Instant::now();
                let result =
                    unsafe { texture.LockRect(0, &mut locked, std::ptr::null(), flags as u32) };
                lock_us += start.elapsed().as_secs_f64() * 1e6;
                if result.is_err() {
                    failures += 1;
                    continue;
                }
                let copy_start = Instant::now();
                unsafe {
                    if streaming {
                        copy_rows_streaming(
                            source.as_ptr(),
                            locked.pBits.cast(),
                            locked.Pitch as usize,
                            row_bytes,
                            h as usize,
                        )
                    } else {
                        copy_rows(
                            source.as_ptr(),
                            locked.pBits.cast(),
                            locked.Pitch as usize,
                            row_bytes,
                            h as usize,
                        )
                    }
                }
                copy_us += copy_start.elapsed().as_secs_f64() * 1e6;
                let unlock_start = Instant::now();
                let _ = unsafe { texture.UnlockRect(0) };
                unlock_us += unlock_start.elapsed().as_secs_f64() * 1e6;
                if frame < 10 {
                    lock_us = 0.0;
                    copy_us = 0.0;
                    unlock_us = 0.0;
                }
            }
            let n = (frames - 10) as f64;
            println!(
                "  {w}x{h} {name}: lock {:.2} copy {:.2} unlock {:.2} ms/frame (failures {failures})",
                lock_us / n / 1000.0,
                copy_us / n / 1000.0,
                unlock_us / n / 1000.0
            );
        }
        drop(texture);
    }
}

unsafe fn open_reader(
    path: &str,
    w: u32,
    h: u32,
    nv12: bool,
    scale: bool,
    attrs: u8,
) -> windows::core::Result<windows::Win32::Media::MediaFoundation::IMFSourceReader> {
    use windows::Win32::Media::MediaFoundation::*;
    let mut attributes = None;
    unsafe { MFCreateAttributes(&mut attributes, 2)? };
    let attributes = attributes.unwrap();
    unsafe {
        if attrs == 1 {
            attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)?;
        } else if attrs == 2 {
            attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)?;
        }
    }
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let reader = unsafe { MFCreateSourceReaderFromURL(PCWSTR(wide.as_ptr()), &attributes)? };
    unsafe {
        reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
        reader.SetStreamSelection(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, true)?;
    }
    let media = unsafe { MFCreateMediaType()? };
    unsafe {
        media.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        if nv12 {
            media.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        } else {
            media.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
        }
        if scale {
            media.SetUINT64(&MF_MT_FRAME_SIZE, ((w as u64) << 32) | h as u64)?;
            media.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1)?;
        }
        media.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        reader.SetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, None, &media)?;
    }
    Ok(reader)
}

fn nv12_to_bgra(
    y_plane: &[u8],
    uv_plane: &[u8],
    stride_y: usize,
    stride_uv: usize,
    w: usize,
    h: usize,
    out: &mut [u8],
) {
    for row in 0..h {
        let y_row = &y_plane[row * stride_y..];
        let uv_row = &uv_plane[(row / 2) * stride_uv..];
        for x in 0..w {
            let c = (y_row[x] as i32 - 16).max(0);
            let d = uv_row[(x / 2) * 2] as i32 - 128;
            let e = uv_row[(x / 2) * 2 + 1] as i32 - 128;
            let r = ((298 * c + 409 * e + 128) >> 8).clamp(0, 255);
            let g = ((298 * c - 100 * d - 208 * e + 128) >> 8).clamp(0, 255);
            let b = ((298 * c + 516 * d + 128) >> 8).clamp(0, 255);
            let i = (row * w + x) * 4;
            out[i] = b as u8;
            out[i + 1] = g as u8;
            out[i + 2] = r as u8;
            out[i + 3] = 255;
        }
    }
}

fn bench_decode(path: &str, nv12: bool, w: u32, h: u32, scale: bool, attrs: u8) {
    use windows::Win32::Media::MediaFoundation::*;
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        MFStartup(MF_VERSION, 0).unwrap();
        {
            let reader = match open_reader(path, w, h, nv12, scale, attrs) {
                Ok(reader) => reader,
                Err(e) => {
                    println!("open {w}x{h} nv12={nv12} scale={scale} attrs={attrs} failed: {e}");
                    return;
                }
            };
            let current = reader
                .GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32)
                .unwrap();
            let size = current.GetUINT64(&MF_MT_FRAME_SIZE).unwrap_or(0);
            let stride = current.GetUINT32(&MF_MT_DEFAULT_STRIDE).unwrap_or(0);
            let fps = current.GetUINT64(&MF_MT_FRAME_RATE).unwrap_or(0);
            let actual_w = (size >> 32) as usize;
            let actual_h = (size & 0xffff_ffff) as usize;
            println!(
                "requested {w}x{h} nv12={nv12} scale={scale}: actual {}x{} stride {} ({}) rate {}/{}",
                actual_w,
                actual_h,
                stride as i32,
                if (stride as i32) < 0 { "bottom-up" } else { "top-down" },
                fps >> 32,
                fps & 0xffff_ffff
            );
            let mut frames = 0u64;
            let mut decode_total = 0f64;
            let mut copy_total = 0f64;
            let mut canvas = vec![0u8; actual_w * actual_h * 4];
            let mut nv12_buf: Vec<u8> = Vec::new();
            let started = Instant::now();
            loop {
                let mut flags = 0u32;
                let mut timestamp = 0i64;
                let mut sample: Option<IMFSample> = None;
                let start = Instant::now();
                let result = reader.ReadSample(
                        MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                        0,
                        None,
                        Some(&mut flags),
                        Some(&mut timestamp),
                    Some(&mut sample),
                );
                if result.is_err() {
                    println!("  read failed");
                    break;
                }
                let decode = start.elapsed().as_secs_f64();
                if flags & (MF_SOURCE_READERF_ENDOFSTREAM.0 as u32) != 0 {
                    break;
                }
                let Some(sample) = sample else { continue };
                let buffer = sample.ConvertToContiguousBuffer().unwrap();
                let mut data = std::ptr::null_mut();
                let mut length = 0u32;
                buffer.Lock(&mut data, None, Some(&mut length)).unwrap();
                let copy_start = Instant::now();
                if nv12 {
                    nv12_buf.resize(length as usize, 0);
                    std::ptr::copy_nonoverlapping(data, nv12_buf.as_mut_ptr(), length as usize);
                } else {
                    let stride_abs = if stride == 0 {
                        actual_w * 4
                    } else {
                        (stride as i32).unsigned_abs() as usize
                    };
                    for y in 0..actual_h {
                        let src_row = if (stride as i32) < 0 {
                            (actual_h - 1 - y) as isize
                        } else {
                            y as isize
                        };
                        std::ptr::copy_nonoverlapping(
                            data.offset(src_row * stride_abs as isize),
                            canvas.as_mut_ptr().add(y * actual_w * 4),
                            (actual_w * 4).min(stride_abs),
                        );
                    }
                }
                let copy = copy_start.elapsed().as_secs_f64();
                buffer.Unlock().unwrap();
                frames += 1;
                if frames > 5 {
                    decode_total += decode;
                    copy_total += copy;
                }
                if frames == 1 {
                    println!("  first sample ts {timestamp} len {length}");
                }
            }
            let elapsed = started.elapsed().as_secs_f64();
            let n = frames.saturating_sub(5).max(1) as f64;
            println!(
                "  decoded {frames} frames in {elapsed:.2}s ({:.1} fps): read {:.2} ms/frame, copy/publish {:.2} ms/frame",
                frames as f64 / elapsed,
                decode_total / n * 1000.0,
                copy_total / n * 1000.0
            );
            if nv12 && !nv12_buf.is_empty() {
                let stride_abs = if stride != 0 {
                    (stride as i32).unsigned_abs() as usize
                } else {
                    actual_w
                };
                let y = &nv12_buf[..stride_abs * actual_h];
                let uv = &nv12_buf[stride_abs * actual_h..];
                let mut convert_total = 0f64;
                for _ in 0..10 {
                    let start = Instant::now();
                    nv12_to_bgra(
                        y,
                        uv,
                        stride_abs,
                        stride_abs,
                        actual_w,
                        actual_h,
                        &mut canvas,
                    );
                    convert_total += start.elapsed().as_secs_f64();
                }
                println!(
                    "  scalar NV12->BGRA at {actual_w}x{actual_h}: {:.2} ms/frame",
                    convert_total / 10.0 * 1000.0
                );
            }
            drop(canvas);
        }
        let _ = MFShutdown();
        CoUninitialize();
    }
}

fn dump_first_frame(path: &str, out: &str, attrs: u8, w: u32, h: u32) {
    use windows::Win32::Media::MediaFoundation::*;
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        MFStartup(MF_VERSION, 0).unwrap();
        let reader = open_reader(path, w, h, false, true, attrs).unwrap();
        let current = reader
            .GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32)
            .unwrap();
        let size = current.GetUINT64(&MF_MT_FRAME_SIZE).unwrap_or(0);
        let stride = current.GetUINT32(&MF_MT_DEFAULT_STRIDE).unwrap_or(0);
        let actual_w = (size >> 32) as usize;
        let actual_h = (size & 0xffff_ffff) as usize;
        let stride_abs = if stride == 0 {
            actual_w * 4
        } else {
            (stride as i32).unsigned_abs() as usize
        };
        let mut flags = 0u32;
        let mut timestamp = 0i64;
        let mut sample: Option<IMFSample> = None;
        reader
            .ReadSample(
                MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                0,
                None,
                Some(&mut flags),
                Some(&mut timestamp),
                Some(&mut sample),
            )
            .unwrap();
        let sample = sample.expect("sample");
        let buffer = sample.ConvertToContiguousBuffer().unwrap();
        let mut data = std::ptr::null_mut();
        buffer.Lock(&mut data, None, None).unwrap();
        let mut bmp = Vec::with_capacity(54 + actual_w * actual_h * 4);
        let file_size = (54 + actual_w * actual_h * 4) as u32;
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&file_size.to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes());
        bmp.extend_from_slice(&54u32.to_le_bytes());
        bmp.extend_from_slice(&40u32.to_le_bytes());
        bmp.extend_from_slice(&(actual_w as i32).to_le_bytes());
        bmp.extend_from_slice(&(-(actual_h as i32)).to_le_bytes());
        bmp.extend_from_slice(&1u16.to_le_bytes());
        bmp.extend_from_slice(&32u16.to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes());
        bmp.extend_from_slice(&((actual_w * actual_h * 4) as u32).to_le_bytes());
        bmp.extend_from_slice(&[0u8; 16]);
        for y in 0..actual_h {
            let row = if (stride as i32) < 0 {
                (actual_h - 1 - y) as isize
            } else {
                y as isize
            };
            bmp.extend_from_slice(std::slice::from_raw_parts(
                data.offset(row * stride_abs as isize),
                actual_w * 4,
            ));
        }
        buffer.Unlock().unwrap();
        std::fs::write(out, &bmp).unwrap();
        println!("wrote {out}: {actual_w}x{actual_h} stride={stride} attrs={attrs} ts={timestamp}");
        let _ = MFShutdown();
        CoUninitialize();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("decode") => {
            let path = args.get(2).expect("video path");
            let nv12 = args.get(3).map(String::as_str) == Some("nv12");
            let w: u32 = args.get(4).map(|s| s.parse().unwrap()).unwrap_or(1920);
            let h: u32 = args.get(5).map(|s| s.parse().unwrap()).unwrap_or(1080);
            let scale = args.get(6).map(String::as_str) == Some("scale");
            let attrs: u8 = args.get(7).map(|s| s.parse().unwrap()).unwrap_or(1);
            bench_decode(path, nv12, w, h, scale, attrs)
        }
        Some("dump") => {
            let path = args.get(2).expect("video path");
            let out = args.get(3).expect("output bmp");
            let attrs: u8 = args.get(4).map(|s| s.parse().unwrap()).unwrap_or(2);
            let w: u32 = args.get(5).map(|s| s.parse().unwrap()).unwrap_or(1920);
            let h: u32 = args.get(6).map(|s| s.parse().unwrap()).unwrap_or(1080);
            dump_first_frame(path, out, attrs, w, h)
        }
        _ => bench_upload(),
    }
}
