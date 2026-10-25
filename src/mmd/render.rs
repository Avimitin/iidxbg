//! D3D9 owner-thread renderer. No file reads, decoding, animation or GPU readback.
use super::*;
use std::ffi::c_void;
use windows::Win32::Graphics::Direct3D9::*;
type GResult<T> = windows::core::Result<T>;
use crate::artwork_shader as transfer_shader;
fn missing() -> windows::core::Error {
    windows::core::Error::from_hresult(windows::core::HRESULT(0x80004005u32 as i32))
}

pub struct Renderer {
    pub target: IDirect3DTexture9,
    depth: IDirect3DSurface9,
    vertices: IDirect3DVertexBuffer9,
    indices: IDirect3DIndexBuffer9,
    textures: Vec<IDirect3DTexture9>,
    model: Arc<Model>,
    pub serial: u64,
    straight: Option<Straight>,
}
struct Straight {
    texture: IDirect3DTexture9,
    shader: IDirect3DPixelShader9,
    serial: u64,
}
/// Restores targets explicitly: D3D9 state blocks exclude render/depth surfaces.
struct Saved {
    device: IDirect3DDevice9,
    state: IDirect3DStateBlock9,
    targets: Vec<Option<IDirect3DSurface9>>,
    depth: Option<IDirect3DSurface9>,
    viewport: D3DVIEWPORT9,
    armed: bool,
}
impl Saved {
    unsafe fn capture(device: &IDirect3DDevice9) -> GResult<Self> {
        let mut caps = D3DCAPS9::default();
        unsafe {
            device.GetDeviceCaps(&mut caps)?;
        }
        let state = unsafe { device.CreateStateBlock(D3DSBT_ALL)? };
        let mut viewport = D3DVIEWPORT9::default();
        unsafe {
            device.GetViewport(&mut viewport)?;
        }
        let targets = (0..caps.NumSimultaneousRTs.min(4))
            .map(|i| unsafe { device.GetRenderTarget(i) }.ok())
            .collect();
        Ok(Self {
            device: device.clone(),
            state,
            targets,
            depth: unsafe { device.GetDepthStencilSurface() }.ok(),
            viewport,
            armed: true,
        })
    }
    unsafe fn restore(&mut self) -> GResult<()> {
        if !std::mem::replace(&mut self.armed, false) {
            return Ok(());
        }
        // Restore everything even if a single device call fails during loss.
        let mut error = None;
        let mut record = |r: GResult<()>| {
            if let Err(e) = r {
                error = Some(e)
            }
        };
        record(unsafe { self.device.SetDepthStencilSurface(None) });
        for (i, target) in self.targets.iter().enumerate() {
            record(unsafe { self.device.SetRenderTarget(i as u32, target.as_ref()) });
        }
        record(unsafe { self.device.SetDepthStencilSurface(self.depth.as_ref()) });
        record(unsafe { self.state.Apply() });
        record(unsafe { self.device.SetViewport(&self.viewport) });
        error.map_or(Ok(()), Err)
    }
}
impl Drop for Saved {
    fn drop(&mut self) {
        let _ = unsafe { self.restore() };
    }
}

unsafe fn upload(device: &IDirect3DDevice9, p: &Prepared) -> GResult<IDirect3DTexture9> {
    let mut texture = None;
    unsafe {
        device.CreateTexture(
            p.width,
            p.height,
            1,
            D3DUSAGE_DYNAMIC as u32,
            D3DFMT_A8R8G8B8,
            D3DPOOL_DEFAULT,
            &mut texture,
            std::ptr::null_mut(),
        )?;
    }
    let texture = texture.ok_or_else(missing)?;
    let mut lock = D3DLOCKED_RECT::default();
    unsafe {
        texture.LockRect(0, &mut lock, std::ptr::null(), D3DLOCK_DISCARD as u32)?;
    }
    if lock.pBits.is_null() || lock.Pitch < (p.width * 4) as i32 {
        let _ = unsafe { texture.UnlockRect(0) };
        return Err(missing());
    }
    for y in 0..p.height as usize {
        unsafe {
            std::ptr::copy_nonoverlapping(
                p.bgra.as_ptr().add(y * p.width as usize * 4),
                (lock.pBits as *mut u8).add(y * lock.Pitch as usize),
                p.width as usize * 4,
            );
        }
    }
    unsafe {
        texture.UnlockRect(0)?;
    }
    Ok(texture)
}
impl Renderer {
    /// Caller must be the graphics owner, including creation and destruction.
    ///
    /// # Safety
    /// The device must be valid and exclusively used on its graphics owner.
    pub unsafe fn new(device: &IDirect3DDevice9, model: Arc<Model>) -> GResult<Self> {
        let mut target = None;
        let mut depth = None;
        let mut vertices = None;
        let mut indices = None;
        let s = &model.settings;
        unsafe {
            device.CreateTexture(
                s.width,
                s.height,
                1,
                D3DUSAGE_RENDERTARGET as u32,
                D3DFMT_A8R8G8B8,
                D3DPOOL_DEFAULT,
                &mut target,
                std::ptr::null_mut(),
            )?;
            device.CreateDepthStencilSurface(
                s.width,
                s.height,
                D3DFMT_D16,
                D3DMULTISAMPLE_NONE,
                0,
                true,
                &mut depth,
                std::ptr::null_mut(),
            )?;
            device.CreateVertexBuffer(
                (model.parsed.geometry.positions.len() / 3 * std::mem::size_of::<Vertex>()) as u32,
                (D3DUSAGE_DYNAMIC | D3DUSAGE_WRITEONLY) as u32,
                0x144,
                D3DPOOL_DEFAULT,
                &mut vertices,
                std::ptr::null_mut(),
            )?;
            device.CreateIndexBuffer(
                (model.parsed.geometry.indices.len() * 4) as u32,
                D3DUSAGE_WRITEONLY as u32,
                D3DFMT_INDEX32,
                D3DPOOL_DEFAULT,
                &mut indices,
                std::ptr::null_mut(),
            )?;
        }
        let indices = indices.ok_or_else(missing)?;
        let mut pointer = std::ptr::null_mut();
        unsafe {
            indices.Lock(0, 0, &mut pointer, 0)?;
        }
        if pointer.is_null() {
            let _ = unsafe { indices.Unlock() };
            return Err(missing());
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                model.parsed.geometry.indices.as_ptr().cast::<u8>(),
                pointer.cast(),
                model.parsed.geometry.indices.len() * 4,
            );
            indices.Unlock()?;
        }
        let white = Prepared {
            label: String::new(),
            width: 1,
            height: 1,
            bgra: vec![255; 4],
        };
        let mut cache = Vec::<(usize, IDirect3DTexture9)>::new();
        let mut textures = Vec::new();
        for image in &model.textures {
            let identity = image.as_ref().map_or(0, |p| Arc::as_ptr(p) as usize);
            let texture = if let Some((_, t)) = cache.iter().find(|(id, _)| *id == identity) {
                t.clone()
            } else {
                let t = unsafe { upload(device, image.as_deref().unwrap_or(&white))? };
                cache.push((identity, t.clone()));
                t
            };
            textures.push(texture);
        }
        Ok(Self {
            target: target.ok_or_else(missing)?,
            depth: depth.ok_or_else(missing)?,
            vertices: vertices.ok_or_else(missing)?,
            indices,
            textures,
            model,
            serial: 0,
            straight: None,
        })
    }
    /// The supplied frame is an immutable completed worker result.
    ///
    /// # Safety
    /// Call on the creation thread with the same device, inside BeginScene/EndScene.
    pub unsafe fn draw(&mut self, device: &IDirect3DDevice9, frame: &Frame) -> GResult<bool> {
        if self.serial == frame.serial {
            return Ok(false);
        }
        if frame.vertices.len() != self.model.parsed.geometry.positions.len() / 3
            || frame.materials.len() != self.textures.len()
        {
            return Err(missing());
        }
        let mut saved = unsafe { Saved::capture(device)? };
        let result = unsafe {
            (|| -> GResult<()> {
                let mut data: *mut c_void = std::ptr::null_mut();
                self.vertices
                    .Lock(0, 0, &mut data, D3DLOCK_DISCARD as u32)?;
                if data.is_null() {
                    let _ = self.vertices.Unlock();
                    return Err(missing());
                }
                std::ptr::copy_nonoverlapping(
                    frame.vertices.as_ptr().cast::<u8>(),
                    data.cast(),
                    frame.vertices.len() * std::mem::size_of::<Vertex>(),
                );
                self.vertices.Unlock()?;
                for i in 1..saved.targets.len() {
                    device.SetRenderTarget(i as u32, None)?;
                }
                device.SetTexture(0, None)?;
                device.SetTexture(1, None)?;
                device.SetDepthStencilSurface(None)?;
                device.SetRenderTarget(0, &self.target.GetSurfaceLevel(0)?)?;
                device.SetDepthStencilSurface(&self.depth)?;
                device.SetViewport(&D3DVIEWPORT9 {
                    Width: self.model.settings.width,
                    Height: self.model.settings.height,
                    MinZ: 0.,
                    MaxZ: 1.,
                    ..Default::default()
                })?;
                device.Clear(
                    0,
                    std::ptr::null(),
                    (D3DCLEAR_TARGET | D3DCLEAR_ZBUFFER) as u32,
                    0,
                    1.,
                    0,
                )?;
                device.SetVertexShader(None)?;
                device.SetPixelShader(None)?;
                device.SetFVF(0x144)?;
                device.SetStreamSource(
                    0,
                    &self.vertices,
                    0,
                    std::mem::size_of::<Vertex>() as u32,
                )?;
                device.SetStreamSourceFreq(0, 1)?;
                device.SetIndices(&self.indices)?;
                for (k, v) in [
                    (D3DRS_ZENABLE, 1),
                    (D3DRS_ZWRITEENABLE, 1),
                    (D3DRS_ZFUNC, D3DCMP_LESSEQUAL.0 as u32),
                    (D3DRS_DEPTHBIAS, 0),
                    (D3DRS_SLOPESCALEDEPTHBIAS, 0),
                    (D3DRS_SHADEMODE, D3DSHADE_GOURAUD.0 as u32),
                    (D3DRS_WRAP0, 0),
                    (D3DRS_WRAP1, 0),
                    (D3DRS_CULLMODE, D3DCULL_NONE.0 as u32),
                    (D3DRS_LIGHTING, 0),
                    (D3DRS_FOGENABLE, 0),
                    (D3DRS_STENCILENABLE, 0),
                    (D3DRS_SCISSORTESTENABLE, 0),
                    (D3DRS_CLIPPLANEENABLE, 0),
                    (D3DRS_FILLMODE, D3DFILL_SOLID.0 as u32),
                    (D3DRS_COLORWRITEENABLE, 15),
                    (D3DRS_SRGBWRITEENABLE, 0),
                    (D3DRS_ALPHATESTENABLE, 1),
                    (D3DRS_ALPHAFUNC, D3DCMP_GREATER.0 as u32),
                    (D3DRS_ALPHAREF, 2),
                    (D3DRS_ALPHABLENDENABLE, 1),
                    (D3DRS_SRCBLEND, D3DBLEND_SRCALPHA.0 as u32),
                    (D3DRS_DESTBLEND, D3DBLEND_INVSRCALPHA.0 as u32),
                    (D3DRS_BLENDOP, D3DBLENDOP_ADD.0 as u32),
                    (D3DRS_SEPARATEALPHABLENDENABLE, 1),
                    (D3DRS_SRCBLENDALPHA, D3DBLEND_ONE.0 as u32),
                    (D3DRS_DESTBLENDALPHA, D3DBLEND_INVSRCALPHA.0 as u32),
                    (D3DRS_BLENDOPALPHA, D3DBLENDOP_ADD.0 as u32),
                ] {
                    device.SetRenderState(k, v)?;
                }
                for (k, v) in [
                    (D3DTSS_COLOROP, D3DTOP_MODULATE.0 as u32),
                    (D3DTSS_COLORARG1, D3DTA_TEXTURE),
                    (D3DTSS_COLORARG2, D3DTA_DIFFUSE),
                    (D3DTSS_ALPHAOP, D3DTOP_MODULATE.0 as u32),
                    (D3DTSS_ALPHAARG1, D3DTA_TEXTURE),
                    (D3DTSS_ALPHAARG2, D3DTA_DIFFUSE),
                    (D3DTSS_TEXCOORDINDEX, 0),
                    (D3DTSS_TEXTURETRANSFORMFLAGS, 0),
                ] {
                    device.SetTextureStageState(0, k, v)?;
                }
                device.SetTexture(1, None)?;
                for (k, v) in [
                    (D3DTSS_COLOROP, D3DTOP_MODULATE.0 as u32),
                    (D3DTSS_COLORARG1, D3DTA_CURRENT),
                    (D3DTSS_COLORARG2, D3DTA_TFACTOR),
                    (D3DTSS_ALPHAOP, D3DTOP_MODULATE.0 as u32),
                    (D3DTSS_ALPHAARG1, D3DTA_CURRENT),
                    (D3DTSS_ALPHAARG2, D3DTA_TFACTOR),
                ] {
                    device.SetTextureStageState(1, k, v)?;
                }
                device.SetTextureStageState(2, D3DTSS_COLOROP, D3DTOP_DISABLE.0 as u32)?;
                for (k, v) in [
                    (D3DSAMP_MINFILTER, D3DTEXF_LINEAR.0 as u32),
                    (D3DSAMP_MAGFILTER, D3DTEXF_LINEAR.0 as u32),
                    (D3DSAMP_MIPFILTER, 0),
                    (D3DSAMP_ADDRESSU, D3DTADDRESS_WRAP.0 as u32),
                    (D3DSAMP_ADDRESSV, D3DTADDRESS_WRAP.0 as u32),
                    (D3DSAMP_SRGBTEXTURE, 0),
                ] {
                    device.SetSamplerState(0, k, v)?;
                }
                for group in &self.model.parsed.geometry.material_groups {
                    let material = &self.model.parsed.materials[group.material_index];
                    device.SetRenderState(
                        D3DRS_CULLMODE,
                        if material.flags.double_sided {
                            D3DCULL_NONE
                        } else {
                            D3DCULL_CCW
                        }
                        .0 as u32,
                    )?;
                    let color = frame.materials[group.material_index];
                    if color[3] <= 0. || group.count == 0 {
                        continue;
                    }
                    let [r, g, b, a] = color.map(|v| (v.clamp(0., 1.) * 255.).round() as u32);
                    device.SetRenderState(
                        D3DRS_TEXTUREFACTOR,
                        (a << 24) | (r << 16) | (g << 8) | b,
                    )?;
                    device.SetTexture(0, &self.textures[group.material_index])?;
                    device.DrawIndexedPrimitive(
                        D3DPT_TRIANGLELIST,
                        0,
                        0,
                        frame.vertices.len() as u32,
                        group.start as u32,
                        (group.count / 3) as u32,
                    )?;
                }
                Ok(())
            })()
        };
        let restored = unsafe { saved.restore() };
        drop(saved);
        result?;
        restored?;
        self.serial = frame.serial;
        Ok(true)
    }

    /// Shared straight-alpha copy for native sprites. Atlas consumers continue
    /// using the premultiplied target. No readback or second model render.
    ///
    /// # Safety
    /// Same graphics-owner and BeginScene/EndScene requirements as `draw`.
    pub unsafe fn straight(&mut self, device: &IDirect3DDevice9) -> GResult<IDirect3DTexture9> {
        let (width, height) = (self.model.settings.width, self.model.settings.height);
        if self.straight.is_none() {
            let mut texture = None;
            unsafe {
                device.CreateTexture(
                    width,
                    height,
                    1,
                    D3DUSAGE_RENDERTARGET as u32,
                    D3DFMT_A8R8G8B8,
                    D3DPOOL_DEFAULT,
                    &mut texture,
                    std::ptr::null_mut(),
                )?;
            }
            self.straight = Some(Straight {
                texture: texture.ok_or_else(missing)?,
                shader: unsafe { device.CreatePixelShader(transfer_shader::CODE.as_ptr())? },
                serial: u64::MAX,
            });
        }
        let output = self.straight.as_mut().unwrap();
        if output.serial != self.serial {
            let mut saved = unsafe { Saved::capture(device)? };
            let result = unsafe {
                (|| -> GResult<()> {
                    for i in 1..saved.targets.len() {
                        device.SetRenderTarget(i as u32, None)?;
                    }
                    device.SetTexture(0, None)?;
                    device.SetTexture(1, None)?;
                    device.SetDepthStencilSurface(None)?;
                    device.SetRenderTarget(0, &output.texture.GetSurfaceLevel(0)?)?;
                    device.SetViewport(&D3DVIEWPORT9 {
                        Width: width,
                        Height: height,
                        MinZ: 0.,
                        MaxZ: 1.,
                        ..Default::default()
                    })?;
                    device.SetVertexShader(None)?;
                    device.SetFVF(0x144)?;
                    device.SetStreamSourceFreq(0, 1)?;
                    device.SetPixelShader(&output.shader)?;
                    for (kind, value) in [
                        (D3DRS_ZENABLE, 0),
                        (D3DRS_ZWRITEENABLE, 0),
                        (D3DRS_CULLMODE, D3DCULL_NONE.0 as u32),
                        (D3DRS_ALPHATESTENABLE, 0),
                        (D3DRS_ALPHABLENDENABLE, 0),
                        (D3DRS_SCISSORTESTENABLE, 0),
                        (D3DRS_COLORWRITEENABLE, 15),
                        (D3DRS_FOGENABLE, 0),
                        (D3DRS_STENCILENABLE, 0),
                        (D3DRS_SRGBWRITEENABLE, 0),
                        (D3DRS_CLIPPLANEENABLE, 0),
                        (D3DRS_FILLMODE, D3DFILL_SOLID.0 as u32),
                        (D3DRS_WRAP0, 0),
                    ] {
                        device.SetRenderState(kind, value)?;
                    }
                    device.SetTexture(0, &self.target)?;
                    device.SetTexture(1, &self.target)?;
                    for stage in [0, 1] {
                        for (kind, value) in [
                            (D3DSAMP_ADDRESSU, D3DTADDRESS_CLAMP.0 as u32),
                            (D3DSAMP_ADDRESSV, D3DTADDRESS_CLAMP.0 as u32),
                            (D3DSAMP_MINFILTER, D3DTEXF_POINT.0 as u32),
                            (D3DSAMP_MAGFILTER, D3DTEXF_POINT.0 as u32),
                            (D3DSAMP_MIPFILTER, 0),
                            (D3DSAMP_SRGBTEXTURE, 0),
                        ] {
                            device.SetSamplerState(stage, kind, value)?;
                        }
                    }
                    device.SetPixelShaderConstantF(0, [0.; 4].as_ptr(), 1)?;
                    device.SetPixelShaderConstantF(
                        8,
                        [0., 0., 1., 1., 1., 1., 0., 0., 0., 0., 1., 0.].as_ptr(),
                        3,
                    )?;
                    let vertices = [[0., 0.], [1., 0.], [0., 1.], [1., 1.]].map(|uv| Vertex {
                        position: [
                            uv[0] * width as f32 - 0.5,
                            uv[1] * height as f32 - 0.5,
                            0.,
                            1.,
                        ],
                        color: 0xffffffff,
                        uv,
                    });
                    device.DrawPrimitiveUP(
                        D3DPT_TRIANGLESTRIP,
                        2,
                        vertices.as_ptr().cast(),
                        std::mem::size_of::<Vertex>() as u32,
                    )
                })()
            };
            let restored = unsafe { saved.restore() };
            result?;
            restored?;
            output.serial = self.serial;
        }
        Ok(output.texture.clone())
    }
}

/// Standalone import preview. Owns a hidden window and a separate device on the
/// caller thread. Readback is for this offline preview only, never the game path.
pub fn preview(model: Arc<Model>, seconds: f32, look: [f32; 2], happy: f32) -> Result<Prepared> {
    use windows::{
        Win32::{Foundation::HWND, UI::WindowsAndMessaging::*},
        core::w,
    };
    struct Window(HWND);
    impl Drop for Window {
        fn drop(&mut self) {
            let _ = unsafe { DestroyWindow(self.0) };
        }
    }
    let frame = model.evaluate(seconds, look, happy)?;
    let width = model.settings.width;
    let height = model.settings.height;
    let graphics = || -> GResult<Prepared> {
        unsafe {
            let window = Window(CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("iidxbg MMD preview"),
                WS_POPUP,
                0,
                0,
                width as i32,
                height as i32,
                None,
                None,
                None,
                None,
            )?);
            let d3d = Direct3DCreate9(D3D_SDK_VERSION).ok_or_else(missing)?;
            let mut params = D3DPRESENT_PARAMETERS {
                BackBufferWidth: width,
                BackBufferHeight: height,
                BackBufferCount: 1,
                BackBufferFormat: D3DFMT_UNKNOWN,
                SwapEffect: D3DSWAPEFFECT_DISCARD,
                hDeviceWindow: window.0,
                Windowed: true.into(),
                PresentationInterval: D3DPRESENT_INTERVAL_IMMEDIATE as u32,
                ..Default::default()
            };
            let mut device = None;
            d3d.CreateDevice(
                D3DADAPTER_DEFAULT,
                D3DDEVTYPE_HAL,
                window.0,
                (D3DCREATE_SOFTWARE_VERTEXPROCESSING | D3DCREATE_FPU_PRESERVE) as u32,
                &mut params,
                &mut device,
            )?;
            let device = device.ok_or_else(missing)?;
            let mut renderer = Renderer::new(&device, model.clone())?;
            device.BeginScene()?;
            let draw = renderer
                .draw(&device, &frame)
                .and_then(|_| renderer.straight(&device));
            let end = device.EndScene();
            let straight = draw?;
            end?;
            let source = straight.GetSurfaceLevel(0)?;
            let mut surface = None;
            device.CreateOffscreenPlainSurface(
                width,
                height,
                D3DFMT_A8R8G8B8,
                D3DPOOL_SYSTEMMEM,
                &mut surface,
                std::ptr::null_mut(),
            )?;
            let surface = surface.ok_or_else(missing)?;
            device.GetRenderTargetData(&source, &surface)?;
            let mut locked = D3DLOCKED_RECT::default();
            surface.LockRect(&mut locked, std::ptr::null(), D3DLOCK_READONLY as u32)?;
            if locked.pBits.is_null() || locked.Pitch < (width * 4) as i32 {
                let _ = surface.UnlockRect();
                return Err(missing());
            }
            let mut bgra = vec![0; (width * height * 4) as usize];
            for y in 0..height as usize {
                std::ptr::copy_nonoverlapping(
                    (locked.pBits as *const u8).add(y * locked.Pitch as usize),
                    bgra.as_mut_ptr().add(y * width as usize * 4),
                    width as usize * 4,
                );
            }
            surface.UnlockRect()?;
            Ok(Prepared {
                label: format!("MMD: {}", model.parsed.metadata.name),
                width,
                height,
                bgra,
            })
        }
    };
    graphics().map_err(|e| Error::Mmd(format!("preview graphics: {e}")))
}
