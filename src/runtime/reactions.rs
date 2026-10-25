//! Read-only exact-build companion signals. docs/iidx34-mmd-reactions.md.
use super::*;
use std::cell::Cell;
thread_local! {
    static SCENE_STATE:Cell<Option<i32>>=const {Cell::new(None)};
    static BPM_SAMPLE:Cell<Option<Instant>>=const {Cell::new(None)};
    static LAST_BPM:Cell<Option<(u32,u32,u32)>>=const {Cell::new(None)};
    static BPM_TRACES:Cell<u8>=const {Cell::new(0)};
}
fn word(object: usize, offset: usize) -> Option<i32> {
    let address = object.checked_add(offset)?;
    if object == 0 || !readable(address) || !readable(address.checked_add(3)?) {
        return None;
    }
    Some(unsafe { (address as *const i32).read_unaligned() })
}
pub(super) fn leave() {
    crate::mmd::ACTIVE.store(false, Ordering::Release);
    crate::mmd::INTERACTIVE.store(false, Ordering::Release);
    crate::mmd::SELECTED_BPM.store(0, Ordering::Release);
    SCENE_STATE.with(|c| c.set(None));
    BPM_SAMPLE.with(|c| c.set(None));
    LAST_BPM.with(|c| c.set(None));
    BPM_TRACES.with(|c| c.set(0));
}
pub(super) unsafe fn selection(n: &Native, scene: usize) {
    if !n.profile.surface_data.contains_key("reaction_scene_state") {
        return;
    }
    if !crate::mmd::ACTIVE.swap(true, Ordering::AcqRel) {
        surfaces::character_changed();
    }
    let native_state = word(scene, n.profile.surface_data["reaction_scene_state"]);
    crate::mmd::INTERACTIVE.store(native_state == Some(3), Ordering::Release);
    if SCENE_STATE.with(|s| s.replace(native_state)) != native_state
        && crate::mmd::snapshot().is_some()
    {
        event(Event::Diag(format!("MMD selection state={native_state:?}")));
    }
    if native_state == Some(3) {
        unsafe { sample_bpm(n, scene) };
    } else {
        crate::mmd::SELECTED_BPM.store(0, Ordering::Release);
        BPM_SAMPLE.with(|c| c.set(None));
    }
}

/// The existing post-original selection callback owns all native reads. Only
/// copied BPM crosses to the animation worker. No I/O, decoding or new hook.
unsafe fn sample_bpm(n: &Native, scene: usize) {
    let now = Instant::now();
    if BPM_SAMPLE.with(|last| {
        if last
            .get()
            .is_some_and(|at| now.duration_since(at) < Duration::from_millis(250))
        {
            true
        } else {
            last.set(Some(now));
            false
        }
    }) {
        return;
    }
    if crate::mmd::snapshot().is_none_or(|m| !m.settings.bpm_sync) {
        crate::mmd::SELECTED_BPM.store(0, Ordering::Release);
        return;
    }
    let value = unsafe { selected_bpm(n, scene) };
    crate::mmd::SELECTED_BPM.store(value.map_or(0, |(_, _, bpm)| bpm), Ordering::Release);
    if LAST_BPM.with(|last| last.replace(value)) != value {
        BPM_TRACES.with(|count| {
            if count.get() < 5 {
                count.set(count.get() + 1);
                event(Event::Diag(match value {
                    Some((id, chart, bpm)) => {
                        format!("MMD selected BPM={bpm} song={id} chart={chart}")
                    }
                    None => "MMD selected BPM unavailable; idle speed 1x".into(),
                }));
            }
        });
    }
}

unsafe fn selected_bpm(n: &Native, scene: usize) -> Option<(u32, u32, u32)> {
    let bar: unsafe extern "system" fn(usize) -> u32 = unsafe { n.function("tempo_bar") };
    let selected: unsafe extern "system" fn(usize) -> usize = unsafe { n.function("tempo_song") };
    let difficulty: unsafe extern "system" fn(usize, u32) -> u32 =
        unsafe { n.function("tempo_difficulty") };
    let side: unsafe extern "system" fn() -> u8 = unsafe { n.function("tempo_player") };
    let style: unsafe extern "system" fn() -> u32 = unsafe { n.function("tempo_style") };
    let layout: unsafe extern "system" fn() -> u8 = unsafe { n.function("tempo_layout") };
    let widget = scene.checked_add(n.profile.surface_data["tempo_widget"])?;
    if unsafe { bar(widget) } != 1 {
        return None;
    }
    let side = unsafe { side() } as u32;
    if side > 1 {
        return None;
    }
    let difficulty = unsafe { difficulty(widget, side) };
    if difficulty > 4 {
        return None;
    }
    // Match the verified selected-chart decoder: 2P SP isn't DP, and DP
    // battle uses SP charts. A nonzero style AND DP layout selects DP records.
    let chart = difficulty
        + if unsafe { style() != 0 && layout() != 0 } {
            5
        } else {
            0
        };
    let song = unsafe { selected(widget) };
    let id = word(song, n.profile.surface_data["tempo_music_id"])? as u32;
    let level_at = song.checked_add(n.profile.surface_data["tempo_levels"] + chart as usize)?;
    if !(1..100_000).contains(&id) || !readable(level_at) {
        return None;
    }
    let level = unsafe { (level_at as *const u8).read() };
    let offset = n.profile.surface_data["tempo_bpm"] + chart as usize * 8;
    let max = word(song, offset)?;
    let min = word(song, offset + 4)?;
    ((1..=12).contains(&level) && (1..=100_000).contains(&max) && (0..=max).contains(&min))
        .then_some((id, chart, max as u32))
}
pub(super) unsafe fn result(n: &Native) {
    if !n.profile.rvas.contains_key("reaction_grade") {
        return;
    }
    let joined: unsafe extern "system" fn(u32) -> u8 = unsafe { n.function("reaction_joined") };
    let grade: unsafe extern "system" fn(u32, u8) -> u32 = unsafe { n.function("reaction_grade") };
    // The native result constructor already calls this same getter after play.
    // 0=AAA, 1=AA, 2=A; no inference from clear lamp or historical best score.
    let won = (0..2).any(|side| unsafe { joined(side) != 0 && grade(side, 0) <= 2 });
    crate::mmd::result(won);
}
