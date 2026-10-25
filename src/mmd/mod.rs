//! User-owned PMX/VMD assets and bounded animation work. No game/GPU objects.
mod deform;
pub(crate) mod input;
#[cfg(windows)]
pub mod render;
#[cfg(test)]
mod tests;
use crate::{Error, Result, pixels::Prepared};
pub use deform::Vertex;
use glam::{Quat, Vec3};
use mmd_anim::{
    format,
    runtime::{AnimationClip, BoneIndex, IkSolveOptions, ModelArena, RuntimeInstance},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub enabled: bool,
    pub model_path: PathBuf,
    pub motion_path: PathBuf,
    pub bpm_sync: bool,
    pub motion_bpm: f32,
    pub scroll_up_motion_path: PathBuf,
    pub scroll_down_motion_path: PathBuf,
    pub happy_motion_path: PathBuf,
    pub other_result_motion_path: PathBuf,
    pub fps: u32,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub yaw: f32,
    pub display_scale: f32,
    pub position_x: f32,
    pub position_y: f32,
    pub keep_hud_clear: bool,
    pub replace_qpro: bool,
    pub qpro_height: f32,
    pub look_bone: String,
    pub look_yaw: f32,
    pub look_hold_seconds: f32,
    pub scratch_reverse: bool,
    pub happy_morph: String,
    pub blink_morph: String,
    pub reactions: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            model_path: PathBuf::new(),
            motion_path: PathBuf::new(),
            bpm_sync: false,
            motion_bpm: 120.,
            scroll_up_motion_path: PathBuf::new(),
            scroll_down_motion_path: PathBuf::new(),
            happy_motion_path: PathBuf::new(),
            other_result_motion_path: PathBuf::new(),
            fps: 30,
            width: 512,
            height: 768,
            scale: 0.85,
            yaw: 0.,
            display_scale: 82.,
            position_x: 50.,
            position_y: 50.,
            keep_hud_clear: true,
            replace_qpro: false,
            qpro_height: 360.,
            look_bone: "頭".into(),
            look_yaw: 45.,
            look_hold_seconds: 1.5,
            scratch_reverse: false,
            happy_morph: "笑い".into(),
            blink_morph: "まばたき".into(),
            reactions: true,
        }
    }
}
impl Settings {
    pub fn motion_paths(&self) -> [&Path; 5] {
        [
            &self.motion_path,
            &self.scroll_up_motion_path,
            &self.scroll_down_motion_path,
            &self.happy_motion_path,
            &self.other_result_motion_path,
        ]
    }
    pub fn validate(&self) -> Result<()> {
        if ![30, 60].contains(&self.fps)
            || !self.motion_bpm.is_finite()
            || !(1.0..=1000.).contains(&self.motion_bpm)
            || !(128..=1024).contains(&self.width)
            || !(128..=1024).contains(&self.height)
            || !self.scale.is_finite()
            || !(0.1..=2.).contains(&self.scale)
            || !self.yaw.is_finite()
            || self.yaw.abs() > 180.
            || !self.display_scale.is_finite()
            || !(10.0..=100.).contains(&self.display_scale)
            || [self.position_x, self.position_y]
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=100.).contains(v))
            || !self.qpro_height.is_finite()
            || !(100.0..=800.).contains(&self.qpro_height)
            || !self.look_yaw.is_finite()
            || !(0.0..=80.).contains(&self.look_yaw)
            || !self.look_hold_seconds.is_finite()
            || !(0.1..=5.).contains(&self.look_hold_seconds)
            || [&self.look_bone, &self.happy_morph, &self.blink_morph]
                .iter()
                .any(|s| s.len() > 256)
        {
            return Err(Error::Config(
                "invalid MMD size, rate, framing or reaction name",
            ));
        }
        Ok(())
    }
}

// Native callbacks publish values, never pointers. No blocking locks or IO.
pub static ACTIVE: AtomicBool = AtomicBool::new(false);
pub static INTERACTIVE: AtomicBool = AtomicBool::new(false);
/// Selected-chart maximum BPM, copied on the native owner; zero means unknown.
pub static SELECTED_BPM: AtomicU32 = AtomicU32::new(0);
static HAPPY: AtomicU64 = AtomicU64::new(0);
pub fn result(happy: bool) {
    HAPPY.store(if happy { 1 } else { 2 }, Ordering::Release);
}
pub fn snapshot() -> Option<Arc<Model>> {
    READY.try_lock().ok()?.clone()
}
static READY: Mutex<Option<Arc<Model>>> = Mutex::new(None);

pub struct Model {
    pub first: Arc<Prepared>,
    pub settings: Settings,
    pub parsed: format::PmxParsedModel,
    pub textures: Vec<Option<Arc<Prepared>>>,
    pub warnings: Vec<String>,
    arena: Arc<ModelArena>,
    clips: Vec<Option<AnimationClip>>,
    head: Option<BoneIndex>,
    smile: Option<u32>,
    blink: Option<u32>,
    center: Vec3,
    extent: f32,
    session: Mutex<Weak<Session>>,
    initial: Arc<Frame>,
}
pub struct Frame {
    pub vertices: Vec<Vertex>,
    pub materials: Vec<[f32; 4]>,
    pub serial: u64,
}
struct Mailbox {
    frame: Mutex<Option<Arc<Frame>>>,
    stop: AtomicBool,
    failed: AtomicBool,
}
pub struct Session {
    mailbox: Arc<Mailbox>,
    worker: thread::Thread,
}
impl Session {
    pub fn failed(&self) -> bool {
        self.mailbox.failed.load(Ordering::Acquire)
    }
    pub fn frame(&self) -> Option<Arc<Frame>> {
        self.mailbox.frame.try_lock().ok()?.clone()
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.mailbox.stop.store(true, Ordering::Release);
        self.worker.unpark();
    }
}

fn err(e: impl std::fmt::Display) -> Error {
    Error::Mmd(e.to_string())
}
fn read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    crate::pixels::read_bounded(fs::File::open(path)?, limit)
}
fn relative(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        root.join(path)
    }
}

/// A model may reference local textures within its extracted package only.
fn texture_path(root: &Path, name: &str) -> Result<PathBuf> {
    let path = PathBuf::from(name.replace('\\', "/"));
    if path.components().any(|c| {
        !matches!(
            c,
            std::path::Component::Normal(_) | std::path::Component::CurDir
        )
    }) {
        return Err(Error::Mmd(
            "texture must be relative to the PMX folder".into(),
        ));
    }
    let path = root.join(path).canonicalize()?;
    if !path.starts_with(root) {
        return Err(Error::Mmd("texture escapes PMX folder".into()));
    }
    Ok(path)
}
fn texture(path: &Path) -> Result<Prepared> {
    let bytes = read(path, 32 * 1024 * 1024)?;
    let format = image::ImageFormat::from_path(path)?;
    if !matches!(
        format,
        image::ImageFormat::Png
            | image::ImageFormat::Jpeg
            | image::ImageFormat::Tga
            | image::ImageFormat::Bmp
    ) {
        return Err(Error::Mmd(
            "PMX texture must be PNG, JPEG, TGA or BMP".into(),
        ));
    }
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let rgba = reader.decode()?.into_rgba8();
    let (width, height) = rgba.dimensions();
    let mut bgra = rgba.into_raw();
    for p in bgra.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
    }
    Ok(Prepared {
        label: path.to_string_lossy().into_owned(),
        width,
        height,
        bgra,
    })
}

pub fn load(settings: &Settings, root: &Path) -> Result<Arc<Model>> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| load_inner(settings, root)))
        .map_err(|_| Error::Mmd("model importer panicked; keeping the previous character".into()))?
}
fn load_inner(settings: &Settings, root: &Path) -> Result<Arc<Model>> {
    settings.validate()?;
    let path = relative(root, &settings.model_path).canonicalize()?;
    let bytes = read(&path, 64 * 1024 * 1024)?;
    let parsed = format::parse_pmx_model(&bytes).map_err(err)?;
    deform::validate(&parsed)?;
    let import = format::import_pmx_runtime(&bytes).map_err(err)?;
    let mut clips = Vec::with_capacity(5);
    for motion_path in settings.motion_paths() {
        if motion_path.as_os_str().is_empty() {
            clips.push(None);
            continue;
        }
        let motion = read(&relative(root, motion_path), 32 * 1024 * 1024)?;
        let motion = format::import_vmd_motion(&motion).map_err(err)?;
        let clip = format::build_mmd_registered_pair_clip(
            &import.model,
            &motion,
            &import.bone_name_to_index,
            &import.morph_name_to_index,
            &import.ik_solver_bone_name_to_index,
            import.model.ik_solvers().len(),
        )
        .map_err(err)?;
        if clip.bone_track_count() == 0 && clip.morph_track_count() == 0 {
            return Err(Error::Mmd(
                "VMD contains no bone or expression tracks matching this model".into(),
            ));
        }
        clips.push(Some(clip));
    }
    let folder = path.parent().ok_or(Error::Config("PMX parent folder"))?;
    let mut loaded = std::collections::HashMap::<PathBuf, Arc<Prepared>>::new();
    let mut textures = Vec::new();
    let mut total = 0usize;
    for material in &parsed.materials {
        if material.texture_path.is_empty() {
            textures.push(None);
            continue;
        }
        let tex_path = texture_path(folder, &material.texture_path)?;
        let tex = if let Some(t) = loaded.get(&tex_path) {
            t.clone()
        } else {
            let t = Arc::new(texture(&tex_path)?);
            total += t.bgra.len();
            if total > 256 * 1024 * 1024 {
                return Err(Error::Limit);
            }
            loaded.insert(tex_path, t.clone());
            t
        };
        textures.push(Some(tex));
    }
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for v in parsed.geometry.positions.as_chunks::<3>().0 {
        let v = Vec3::from_slice(v);
        min = min.min(v);
        max = max.max(v);
    }
    let center = (min + max) * 0.5;
    let aspect = settings.width as f32 / settings.height as f32;
    let extent = (max.y - min.y).max((max.x - min.x) / aspect).max(0.001);
    let mut warnings = Vec::new();
    if !parsed.rigid_bodies.is_empty() {
        warnings.push("Rigid-body physics is not simulated in this version".into());
    }
    if !parsed.soft_bodies.is_empty() {
        warnings.push("Soft bodies are not simulated".into());
    }
    if parsed
        .materials
        .iter()
        .any(|m| !m.sphere_texture_path.is_empty() || !m.toon_texture_path.is_empty())
    {
        warnings.push(
            "Uses simple toon lighting; custom sphere/toon maps and MME effects are not rendered"
                .into(),
        );
    }
    let head = parsed
        .skeleton
        .bones
        .iter()
        .position(|b| {
            !settings.look_bone.is_empty()
                && (b.name == settings.look_bone || b.english_name == settings.look_bone)
        })
        .map(|i| BoneIndex(i as u32));
    let find_morph = |name: &str| {
        parsed
            .morphs
            .iter()
            .position(|m| !name.is_empty() && (m.name == name || m.english_name == name))
            .map(|i| i as u32)
    };
    let smile = find_morph(&settings.happy_morph);
    let blink = find_morph(&settings.blink_morph);
    if settings.reactions && head.is_none() {
        warnings.push("Look bone not found; head tracking disabled".into());
    }
    if settings.reactions && smile.is_none() {
        warnings.push("Happy morph not found; result expression disabled".into());
    }
    let mut model = Arc::new(Model {
        first: Arc::new(Prepared {
            label: path.to_string_lossy().into_owned(),
            width: settings.width,
            height: settings.height,
            bgra: vec![0; (settings.width * settings.height * 4) as usize],
        }),
        settings: settings.clone(),
        parsed,
        textures,
        warnings,
        arena: Arc::new(import.model),
        clips,
        head,
        smile,
        blink,
        center,
        extent,
        session: Mutex::new(Weak::new()),
        initial: Arc::new(Frame {
            vertices: Vec::new(),
            materials: Vec::new(),
            serial: 1,
        }),
    });
    let first = model.evaluate(0., [0., 0.], 0.)?;
    Arc::get_mut(&mut model).unwrap().initial = Arc::new(first);
    Ok(model)
}

impl Model {
    pub fn lease(self: &Arc<Self>) -> Result<Arc<Session>> {
        let mut lease = self
            .session
            .try_lock()
            .map_err(|_| Error::Native("MMD session busy"))?;
        if let Some(session) = lease.upgrade() {
            return Ok(session);
        }
        let mailbox = Arc::new(Mailbox {
            frame: Mutex::new(Some(self.initial.clone())),
            stop: AtomicBool::new(false),
            failed: AtomicBool::new(false),
        });
        let model = self.clone();
        let shared = mailbox.clone();
        let join = thread::Builder::new()
            .name("iidxbg-mmd".into())
            .spawn(move || {
                // This thread owns all animation state. It never touches game memory.
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    animate(model, &shared)
                }))
                .is_err()
                {
                    shared.failed.store(true, Ordering::Release);
                }
            })?;
        let session = Arc::new(Session {
            mailbox,
            worker: join.thread().clone(),
        });
        *lease = Arc::downgrade(&session);
        Ok(session)
    }
    pub fn evaluate(&self, seconds: f32, look: [f32; 2], happy: f32) -> Result<Frame> {
        let mut evaluator = Evaluator::new(self);
        evaluator.frame(
            self,
            Sample {
                motion: Motion::Normal,
                seconds,
            },
            look,
            happy,
            1,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Motion {
    Normal,
    ScrollUp,
    ScrollDown,
    Happy,
    OtherResult,
}
impl Motion {
    fn is_result(self) -> bool {
        matches!(self, Self::Happy | Self::OtherResult)
    }
}
#[derive(Clone, Copy)]
struct Sample {
    motion: Motion,
    seconds: f32,
}

fn idle_rate(settings: &Settings, selected_bpm: u32) -> f64 {
    if !settings.bpm_sync || !(1..=100_000).contains(&selected_bpm) {
        return 1.;
    }
    (selected_bpm as f64 / settings.motion_bpm as f64).clamp(0.25, 4.)
}

/// Integrate speed changes without jumping to a different pose. One-shot
/// reactions keep their wall-clock duration; the idle timeline keeps advancing.
#[derive(Default)]
struct IdleClock {
    seconds: f64,
}
impl IdleClock {
    fn sample(&mut self, model: &Model, dt: f32, bpm: u32, mut sample: Sample) -> Sample {
        let duration = model.clips[Motion::Normal as usize]
            .as_ref()
            .and_then(AnimationClip::frame_range)
            .map_or(1., |(_, end)| (end as f64 + 1.) / 30.);
        self.seconds = (self.seconds + dt as f64 * idle_rate(&model.settings, bpm)) % duration;
        if sample.motion == Motion::Normal {
            sample.seconds = self.seconds as f32;
        }
        sample
    }
}
/// One-shot result motions take priority over scrolling. Repeated scrolling
/// does not restart the same clip every input tick. Normal dance keeps its time.
#[derive(Default)]
struct Reactions {
    current: Option<(Motion, f32)>,
}
impl Reactions {
    fn sample(&mut self, model: &Model, now: f32, scroll: i32, result: u64) -> Sample {
        if let Some((kind, start)) = self.current {
            let duration = model.clips[kind as usize]
                .as_ref()
                .and_then(AnimationClip::frame_range)
                .map_or(0., |(_, end)| (end as f32 + 1.) / 30.);
            if now - start >= duration {
                self.current = None;
            }
        }
        let requested = match result {
            1 => Some(Motion::Happy),
            2 => Some(Motion::OtherResult),
            _ if scroll < 0 => Some(Motion::ScrollUp),
            _ if scroll > 0 => Some(Motion::ScrollDown),
            _ => None,
        };
        if model.settings.reactions {
            if let Some(kind) = requested
                && model.clips[kind as usize].is_some()
                && self
                    .current
                    .is_none_or(|(old, _)| kind.is_result() || (!old.is_result() && old != kind))
            {
                self.current = Some((kind, now));
            }
        } else {
            self.current = None;
        }
        self.current.map_or(
            Sample {
                motion: Motion::Normal,
                seconds: now,
            },
            |(motion, start)| Sample {
                motion,
                seconds: now - start,
            },
        )
    }
}
struct Evaluator {
    runtime: RuntimeInstance,
}
impl Evaluator {
    fn new(model: &Model) -> Self {
        Self {
            runtime: RuntimeInstance::new(model.arena.clone()),
        }
    }
    fn frame(
        &mut self,
        model: &Model,
        sample: Sample,
        look: [f32; 2],
        happy: f32,
        serial: u64,
    ) -> Result<Frame> {
        let clip = model.clips[sample.motion as usize].as_ref();
        let seconds = sample.seconds;
        let frame = clip
            .and_then(AnimationClip::frame_range)
            .map_or(0., |(_, end)| (seconds * 30.) % (end as f32 + 1.));
        let mut ids = Vec::with_capacity(2);
        let mut weights = Vec::with_capacity(2);
        let phase = seconds % 4.3;
        if phase < 0.24
            && let Some(id) = model.blink
        {
            ids.push(id);
            weights.push((1. - (phase - 0.12).abs() / 0.12).clamp(0., 1.));
        }
        if happy > 0.
            && let Some(id) = model.smile
        {
            ids.push(id);
            weights.push(happy);
        }
        let options = IkSolveOptions {
            max_iterations_cap: Some(64),
            ..Default::default()
        };
        self.runtime
            .evaluate_with_morph_overrides(clip, frame, &ids, &weights, options, true)
            .map_err(err)?;
        if model.settings.reactions
            && sample.motion == Motion::Normal
            && look != [0., 0.]
            && let Some(head) = model.head
        {
            let current = self.runtime.pose().local_rotation(head);
            let parent = model.parsed.skeleton.bones[head.0 as usize].parent_index;
            let parent_rotation = if parent >= 0 {
                self.runtime.pose().world_matrices()[parent as usize]
                    .to_scale_rotation_translation()
                    .1
            } else {
                Quat::IDENTITY
            };
            let rotation = aim_head(
                current,
                parent_rotation,
                self.runtime.append_rotation(head),
                model.settings.yaw.to_radians(),
                look,
                model.settings.look_yaw.to_radians(),
            );
            self.runtime.pose_mut().set_local_rotation(head, rotation);
            self.runtime.evaluate_current_pose_with_ik_options(options);
        }
        deform::frame(model, &self.runtime, serial)
    }
}

/// Aim in screen/model world space so the dance's head/parent rotation cannot
/// cancel a small additive yaw. Blend in/out, limiting the local neck rotation.
fn aim_head(
    current: Quat,
    parent: Quat,
    append: Quat,
    model_yaw: f32,
    look: [f32; 2],
    angle: f32,
) -> Quat {
    if angle <= 0.0001 {
        return current;
    }
    let blend = (look[0].abs() / angle).clamp(0., 1.);
    let world = Quat::from_rotation_y(look[0].signum() * angle - model_yaw)
        * Quat::from_rotation_x(look[1]);
    let local = (parent.inverse() * world).normalize();
    let local = if local.w < 0. { -local } else { local };
    let limit = 80f32.to_radians();
    let delta = local.to_scaled_axis();
    let limited = Quat::from_scaled_axis(delta.clamp_length_max(limit));
    // PMX applies the inherited grant after the bone's own rotation:
    // effective_local = local * append. Aim the effective rotation, then
    // remove the grant so reevaluation does not apply it twice.
    (current * append)
        .slerp(limited, blend)
        .mul_quat(append.inverse())
        .normalize()
}
fn animate(model: Arc<Model>, mailbox: &Mailbox) {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::System::Threading::SetThreadPriority(
            windows_sys::Win32::System::Threading::GetCurrentThread(),
            windows_sys::Win32::System::Threading::THREAD_PRIORITY_BELOW_NORMAL,
        );
    }
    let mut evaluator = Evaluator::new(&model);
    let mut serial = 1;
    let mut seconds = 0.;
    let mut last = Instant::now();
    let mut look = [0., 0.];
    let mut scratch = input::Scratch::default();
    let mut look_until = 0.;
    let mut happy_until = 0.;
    let mut direction = 0.;
    let mut reactions = Reactions::default();
    let mut idle_clock = IdleClock::default();
    while !mailbox.stop.load(Ordering::Acquire) {
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f32().min(0.1);
        last = now;
        if !ACTIVE.load(Ordering::Acquire) {
            scratch.reset();
            reactions = Reactions::default();
            look = [0., 0.];
            look_until = 0.;
            happy_until = 0.;
            thread::park_timeout(Duration::from_millis(100));
            continue;
        }
        seconds += dt;
        let scroll = if model.settings.reactions && INTERACTIVE.load(Ordering::Acquire) {
            scratch.poll(model.settings.scratch_reverse)
        } else {
            scratch.reset();
            0
        };
        let scrolled = scroll != 0;
        if scrolled {
            direction = scroll as f32;
            look_until = seconds + model.settings.look_hold_seconds;
        }
        let won = HAPPY.swap(0, Ordering::AcqRel);
        if won != 0 {
            happy_until = if won == 1 { seconds + 6. } else { 0. };
        }
        let target = if seconds < look_until {
            [-model.settings.look_yaw.to_radians(), direction * 0.12]
        } else {
            [0., 0.]
        };
        for i in 0..2 {
            look[i] += (target[i] - look[i]) * (1. - (-dt * 8.).exp());
        }
        serial += 1;
        let smile = if model.settings.reactions {
            (happy_until - seconds).clamp(0., 1.)
        } else {
            0.
        };
        let sample = reactions.sample(
            &model,
            seconds,
            if scrolled { direction as i32 } else { 0 },
            won,
        );
        let sample = idle_clock.sample(&model, dt, SELECTED_BPM.load(Ordering::Acquire), sample);
        match evaluator.frame(&model, sample, look, smile, serial) {
            Ok(frame) => {
                if let Ok(mut output) = mailbox.frame.lock() {
                    *output = Some(Arc::new(frame));
                }
            }
            Err(_) => {
                mailbox.failed.store(true, Ordering::Release);
                break;
            }
        }
        if let Some(wait) =
            Duration::from_secs_f64(1. / model.settings.fps as f64).checked_sub(now.elapsed())
        {
            thread::park_timeout(wait);
        }
    }
}

#[derive(Default)]
pub struct Loader {
    last: Option<(Settings, Vec<Dependency>)>,
}
type Dependency = (PathBuf, u64, SystemTime);
impl Loader {
    /// Scene-request worker only. All dependencies are rechecked at request boundaries.
    pub fn refresh(&mut self, settings: &Settings, root: &Path, enabled: bool) -> Result<bool> {
        if !enabled || !settings.enabled || settings.model_path.as_os_str().is_empty() {
            self.last = None;
            return Ok(READY
                .lock()
                .map_err(|_| Error::Native("MMD publication"))?
                .take()
                .is_some());
        }
        if let Some((old, files)) = &self.last
            && old == settings
            && files.iter().all(|(p, size, time)| {
                fs::metadata(p).is_ok_and(|m| m.len() == *size && m.modified().ok() == Some(*time))
            })
        {
            return Ok(false);
        }
        let model = load(settings, root)?;
        let mut paths = vec![relative(root, &settings.model_path)];
        for motion in settings.motion_paths() {
            if !motion.as_os_str().is_empty() {
                paths.push(relative(root, motion));
            }
        }
        paths.extend(
            model
                .textures
                .iter()
                .flatten()
                .map(|t| PathBuf::from(&t.label)),
        );
        paths.sort();
        paths.dedup();
        let mut files = Vec::new();
        for p in paths {
            let m = fs::metadata(&p)?;
            files.push((p, m.len(), m.modified()?));
        }
        self.last = Some((settings.clone(), files));
        *READY.lock().map_err(|_| Error::Native("MMD publication"))? = Some(model);
        Ok(true)
    }
}
