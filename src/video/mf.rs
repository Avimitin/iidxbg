//! Windows Media Foundation objects live and die on this thread (MTA).
use super::*;
use crate::{Error, Result};
use std::{path::Path, sync::mpsc};
use windows::{
    Win32::{
        Media::MediaFoundation::*,
        System::Com::{
            COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize, StructuredStorage::PROPVARIANT,
        },
    },
    core::{Interface, PCWSTR},
};

const STREAM: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
fn err(e: windows::core::Error) -> Error {
    // Keep the HRESULT even when Windows supplies a localized message. The UI
    // can then explain missing codecs without matching translated OS strings.
    Error::Video(format!("{} (0x{:08X})", e.message(), e.code().0 as u32))
}
struct Platform;
impl Platform {
    fn start() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED)
                .ok()
                .map_err(err)?;
            if let Err(e) = MFStartup(MF_VERSION, MFSTARTUP_FULL) {
                CoUninitialize();
                return Err(err(e));
            }
        }
        Ok(Self)
    }
}
impl Drop for Platform {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
            CoUninitialize();
        }
    }
}

struct Reader {
    reader: IMFSourceReader,
    width: u32,
    height: u32,
    stride: i32,
    label: String,
}
impl Reader {
    fn open(path: &Path, width: u32, height: u32) -> Result<Self> {
        use std::os::windows::ffi::OsStrExt;
        if width == 0 || height == 0 || width > 1920 || height > 1080 {
            return Err(Error::Limit);
        }
        // No URLs or network providers: use the selected filesystem path.
        if !path.is_file() {
            return Err(Error::Video("video file does not exist".into()));
        }
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        if wide[..wide.len() - 1].contains(&0) {
            return Err(Error::Format);
        }
        unsafe {
            let mut attrs = None;
            MFCreateAttributes(&mut attrs, 1).map_err(err)?;
            let attrs = attrs.ok_or(Error::Native("MF attributes"))?;
            attrs
                .SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)
                .map_err(err)?;
            let reader = MFCreateSourceReaderFromURL(PCWSTR(wide.as_ptr()), &attrs).map_err(err)?;
            reader
                .SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)
                .map_err(err)?;
            reader.SetStreamSelection(STREAM, true).map_err(err)?;
            let source = reader.GetNativeMediaType(STREAM, 0).map_err(err)?;
            let size = source.GetUINT64(&MF_MT_FRAME_SIZE).map_err(err)?;
            if (size >> 32) * (size & 0xffffffff) > 64_000_000 {
                return Err(Error::Limit);
            }
            let media = MFCreateMediaType().map_err(err)?;
            media
                .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
                .map_err(err)?;
            media
                .SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)
                .map_err(err)?;
            media
                .SetUINT64(&MF_MT_FRAME_SIZE, ((width as u64) << 32) | height as u64)
                .map_err(err)?;
            media
                .SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1)
                .map_err(err)?;
            media
                .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
                .map_err(err)?;
            reader
                .SetCurrentMediaType(STREAM, None, &media)
                .map_err(err)?;
            let actual = reader.GetCurrentMediaType(STREAM).map_err(err)?;
            if actual.GetUINT64(&MF_MT_FRAME_SIZE).map_err(err)?
                != ((width as u64) << 32) | height as u64
            {
                return Err(Error::Video("decoder did not accept canvas size".into()));
            }
            let stride = match actual.GetUINT32(&MF_MT_DEFAULT_STRIDE) {
                Ok(value) => value as i32,
                Err(_) => {
                    MFGetStrideForBitmapInfoHeader(MFVideoFormat_RGB32.data1, width).map_err(err)?
                }
            };
            Ok(Self {
                reader,
                width,
                height,
                stride,
                label: format!("video:{}", path.display()),
            })
        }
    }

    fn rewind(&self) -> Result<()> {
        unsafe {
            self.reader
                .SetCurrentPosition(&windows::core::GUID::zeroed(), &PROPVARIANT::from(0i64))
        }
        .map_err(err)
    }

    fn read(&mut self) -> Result<Option<(i64, i64, Arc<Prepared>)>> {
        // Reject streams that make no progress rather than spinning forever.
        for _ in 0..128 {
            unsafe {
                let mut flags = 0;
                let mut pts = 0;
                let mut sample = None;
                self.reader
                    .ReadSample(
                        STREAM,
                        0,
                        None,
                        Some(&mut flags),
                        Some(&mut pts),
                        Some(&mut sample),
                    )
                    .map_err(err)?;
                if flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 != 0 {
                    // Decoders often refine stride on the very first sample.
                    // Accept compatible layout updates, reject canvas/subtype changes.
                    let actual = self.reader.GetCurrentMediaType(STREAM).map_err(err)?;
                    if actual.GetGUID(&MF_MT_SUBTYPE).map_err(err)? != MFVideoFormat_RGB32
                        || actual.GetUINT64(&MF_MT_FRAME_SIZE).map_err(err)?
                            != ((self.width as u64) << 32) | self.height as u64
                    {
                        return Err(Error::Video("incompatible video format change".into()));
                    }
                    self.stride = match actual.GetUINT32(&MF_MT_DEFAULT_STRIDE) {
                        Ok(value) => value as i32,
                        Err(_) => {
                            MFGetStrideForBitmapInfoHeader(MFVideoFormat_RGB32.data1, self.width)
                                .map_err(err)?
                        }
                    };
                }
                if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    return Ok(None);
                }
                let Some(sample) = sample else { continue };
                let duration = sample.GetSampleDuration().unwrap_or(333333).max(1);
                let buffer = sample.ConvertToContiguousBuffer().map_err(err)?;
                let bgra = if let Ok(buffer2d) = buffer.cast::<IMF2DBuffer2>() {
                    let mut scanline = std::ptr::null_mut();
                    let mut start = std::ptr::null_mut();
                    let mut pitch = 0;
                    let mut length = 0;
                    buffer2d
                        .Lock2DSize(
                            MF2DBuffer_LockFlags_Read,
                            &mut scanline,
                            &mut pitch,
                            &mut start,
                            &mut length,
                        )
                        .map_err(err)?;
                    // The actual 2D pitch can differ in sign from the media type's
                    // default (the processor may return a top-down surface).
                    let result = copy_scanlines(
                        start,
                        length as usize,
                        scanline,
                        self.width,
                        self.height,
                        pitch,
                    );
                    let unlock = buffer2d.Unlock2D().map_err(err);
                    let pixels = result?;
                    unlock?;
                    pixels
                } else {
                    let mut data = std::ptr::null_mut();
                    let mut length = 0;
                    buffer
                        .Lock(&mut data, None, Some(&mut length))
                        .map_err(err)?;
                    // Every exit after Lock must unlock, including malformed rows.
                    let result =
                        copy_bgra(data, length as usize, self.width, self.height, self.stride);
                    let unlock = buffer.Unlock().map_err(err);
                    let bgra = result?;
                    unlock?;
                    bgra
                };
                return Ok(Some((
                    pts,
                    duration,
                    Arc::new(Prepared {
                        label: self.label.clone(),
                        width: self.width,
                        height: self.height,
                        bgra,
                    }),
                )));
            }
        }
        Err(Error::Video("decoder produced no video samples".into()))
    }
}

unsafe fn copy_bgra(
    data: *const u8,
    length: usize,
    width: u32,
    height: u32,
    stride: i32,
) -> Result<Vec<u8>> {
    let offset = if stride < 0 {
        (stride.unsigned_abs() as usize)
            .checked_mul(height.saturating_sub(1) as usize)
            .ok_or(Error::Limit)?
    } else {
        0
    };
    if offset >= length || data.is_null() {
        return Err(Error::Video("invalid decoded video buffer".into()));
    }
    unsafe { copy_scanlines(data, length, data.add(offset), width, height, stride) }
}

unsafe fn copy_scanlines(
    data: *const u8,
    length: usize,
    scanline: *const u8,
    width: u32,
    height: u32,
    stride: i32,
) -> Result<Vec<u8>> {
    let row = width as usize * 4;
    let pitch = stride.unsigned_abs() as usize;
    let first = (scanline as usize)
        .checked_sub(data as usize)
        .ok_or(Error::Limit)?;
    let last = first as i128 + i128::from(stride) * i128::from(height.saturating_sub(1));
    if data.is_null()
        || height == 0
        || pitch < row
        || last < 0
        || (first as i128).max(last) + row as i128 > length as i128
    {
        return Err(Error::Video("invalid decoded video stride/buffer".into()));
    }
    let mut bgra = vec![0; row * height as usize];
    for y in 0..height as usize {
        let offset = (first as i128 + stride as i128 * y as i128) as usize;
        unsafe {
            std::ptr::copy_nonoverlapping(data.add(offset), bgra.as_mut_ptr().add(y * row), row);
        }
    }
    // RGB32's fourth channel is unspecified, not an alpha channel.
    for pixel in bgra.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
    }
    Ok(bgra)
}

/// Called only by the provider worker. First frame must be ready before the
/// candidate can be published for a future visit. Timeout never blocks rendering.
pub fn load(path: &Path, width: u32, height: u32) -> Result<Arc<Video>> {
    let path = path.to_owned();
    let (tx, rx) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("iidxbg-video".into())
        .spawn(move || {
            unsafe {
                use windows_sys::Win32::System::Threading::{
                    GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
                };
                SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
            }
            let setup = || -> Result<_> {
                let platform = Platform::start()?;
                let mut reader = Reader::open(&path, width, height)?;
                let (_, _, first) = reader.read()?.ok_or(Error::Video("empty video".into()))?;
                Ok((platform, reader, Arc::new(Video::new(first))))
            };
            let (platform, mut reader, video) = match setup() {
                Ok(v) => v,
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            };
            let _ = video.worker.set(thread::current());
            let weak = Arc::downgrade(&video);
            if tx.send(Ok(video)).is_err() {
                return;
            }
            let mut epoch = 0;
            let mut origin = None;
            let mut loop_offset = 0i64;
            let mut end = 0i64;
            let mut empty_loop = false;
            loop {
                let Some(video) = weak.upgrade() else { break };
                let requested = video.requested.load(Ordering::Acquire);
                let result = (|| -> Result<bool> {
                    if requested == 0 {
                        return Ok(false);
                    }
                    if epoch != requested {
                        reader.rewind()?;
                        video
                            .queue
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .clear();
                        epoch = requested;
                        origin = None;
                        loop_offset = 0;
                        end = 0;
                        empty_loop = false;
                    }
                    if video.queue.lock().unwrap_or_else(|e| e.into_inner()).len() >= CAPACITY {
                        return Ok(false);
                    }
                    let Some((pts, duration, pixels)) = reader.read()? else {
                        if empty_loop {
                            return Err(Error::Video("empty video loop".into()));
                        }
                        reader.rewind()?;
                        loop_offset = end;
                        origin = None;
                        empty_loop = true;
                        return Ok(true);
                    };
                    empty_loop = false;
                    let relative = pts.saturating_sub(*origin.get_or_insert(pts)).max(0);
                    let timestamp = loop_offset.saturating_add(relative);
                    end = end.max(timestamp.saturating_add(duration));
                    if video.requested.load(Ordering::Acquire) == epoch {
                        video
                            .queue
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .push_back(Frame {
                                epoch,
                                pts: Duration::from_nanos((timestamp as u64).saturating_mul(100)),
                                pixels,
                            });
                    }
                    Ok(true)
                })();
                let progress = match result {
                    Ok(v) => v,
                    Err(e) => {
                        *video.error.lock().unwrap_or_else(|e| e.into_inner()) =
                            Some(e.to_string());
                        break;
                    }
                };
                // The worker must not retain the last strong reference while idle.
                drop(video);
                if !progress {
                    thread::park_timeout(Duration::from_millis(100));
                }
            }
            drop(reader);
            drop(platform);
        })?;
    rx.recv_timeout(Duration::from_secs(20))
        .map_err(|_| Error::Video("video preparation timed out".into()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    #[test]
    fn padded_bottom_up_rows_and_rgb32_alpha() {
        let data = [1, 2, 3, 0, 0, 0, 0, 0, 4, 5, 6, 0];
        let pixels = unsafe { copy_bgra(data.as_ptr(), data.len(), 1, 2, -8) }.unwrap();
        assert_eq!(pixels, [4, 5, 6, 255, 1, 2, 3, 255]);
        assert!(unsafe { copy_bgra(data.as_ptr(), 8, 1, 2, -8) }.is_err());
        assert!(unsafe { copy_bgra(data.as_ptr(), 12, 2, 2, 4) }.is_err());
        // Lock2DSize supplies scanline zero separately from the allocation base.
        assert_eq!(
            unsafe { copy_scanlines(data.as_ptr(), 12, data.as_ptr().add(8), 1, 2, -8) }.unwrap(),
            pixels
        );
        assert!(unsafe { copy_scanlines(data.as_ptr(), 12, data.as_ptr(), 1, 2, -8) }.is_err());
        assert_eq!(
            unsafe { copy_scanlines(data.as_ptr(), 12, data.as_ptr(), 1, 2, 8) }.unwrap(),
            [1, 2, 3, 255, 4, 5, 6, 255]
        );
    }
    #[test]
    #[ignore = "requires IIDXBG_TEST_STRIPES: red top half, blue bottom half H.264 fixture"]
    fn real_decoder_preserves_top_down_orientation() {
        let path = std::env::var_os("IIDXBG_TEST_STRIPES").expect("fixture path");
        let video = load(Path::new(&path), 320, 180).unwrap();
        let top = &video.first.bgra[(45 * 320 + 160) * 4..][..4];
        let bottom = &video.first.bgra[(135 * 320 + 160) * 4..][..4];
        assert!(top[2] > 200 && top[0] < 40, "top must be red: {top:?}");
        assert!(
            bottom[0] > 200 && bottom[2] < 40,
            "bottom must be blue: {bottom:?}"
        );
    }

    #[test]
    #[ignore = "requires a local Media Foundation fixture; set IIDXBG_TEST_VIDEO"]
    fn real_decoder_loops_and_restarts() {
        let path = std::env::var_os("IIDXBG_TEST_VIDEO").expect("fixture path");
        let video = load(Path::new(&path), 320, 180).unwrap();
        assert_eq!(video.first.bgra.len(), 320 * 180 * 4);
        for _ in 0..2 {
            let epoch = video.begin();
            let start = Instant::now();
            let mut frames = 0;
            while start.elapsed() < Duration::from_secs(12) {
                if video.frame(epoch, start.elapsed()).is_some() {
                    frames += 1;
                }
                thread::sleep(Duration::from_millis(5));
            }
            assert!(
                frames > 300,
                "only {frames} frames; error {:?}",
                video.take_error()
            );
            video.stop();
        }
    }
}
