//! Read-only Spice SDK v0.1 input, sampled on the animation worker.
//! ABI/source evidence: docs/iidx34-mmd-reactions.md.
use std::sync::atomic::{AtomicUsize, Ordering};

static ANALOG: AtomicUsize = AtomicUsize::new(0);
static BUTTON: AtomicUsize = AtomicUsize::new(0);
type GetAnalog = unsafe extern "C" fn(u32, *mut f32) -> i32;
type GetButton = unsafe extern "C" fn(u32, *mut bool, *mut f32) -> i32;

/// Function addresses supplied by successful Spice SDK initialization only.
/// Zero disables reads during shutdown. No game or graphics pointers are stored.
pub(crate) fn configure(analog: usize, button: usize) {
    ANALOG.store(analog, Ordering::Release);
    BUTTON.store(button, Ordering::Release);
}

#[derive(Default)]
struct Axis {
    anchor: Option<f32>,
}
impl Axis {
    fn sample(&mut self, value: Option<f32>) -> i32 {
        let Some(value) = value.filter(|v| v.is_finite() && (0.0..=1.0).contains(v)) else {
            self.anchor = None;
            return 0;
        };
        let Some(anchor) = self.anchor else {
            self.anchor = Some(value);
            return 0;
        };
        let delta = (value - anchor + 0.5).rem_euclid(1.) - 0.5;
        // Accumulate small movement; reject jitter and discontinuities.
        if delta.abs() < 0.004 {
            return 0;
        }
        self.anchor = Some(value);
        if delta.abs() > 0.25 {
            return 0;
        }
        if delta < 0. { -1 } else { 1 }
    }
}

#[derive(Default)]
pub(super) struct Scratch {
    axes: [Axis; 2],
    traces: u8,
}
impl Scratch {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn poll(&mut self, reverse: bool) -> i32 {
        let analog = ANALOG.load(Ordering::Acquire);
        let button = BUTTON.load(Ordering::Acquire);
        let mut direction = 0;
        let mut source_side = 0;
        for side in 0..2 {
            let value = if analog != 0 {
                let get: GetAnalog = unsafe { std::mem::transmute(analog) };
                let mut value = 0.;
                (unsafe { get(side as u32, &mut value) } == 0).then_some(value)
            } else {
                None
            };
            let mut moved = self.axes[side].sample(value);
            if button != 0 {
                let get: GetButton = unsafe { std::mem::transmute(button) };
                let mut plus = false;
                let mut minus = false;
                // SDK named IDs: IIDX_Button_P{1,2}_TTPlus / TTMinus.
                let first = if side == 0 { 10 } else { 22 };
                unsafe {
                    if get(first, &mut plus, std::ptr::null_mut()) != 0 {
                        plus = false;
                    }
                    if get(first + 1, &mut minus, std::ptr::null_mut()) != 0 {
                        minus = false;
                    }
                }
                if plus != minus {
                    moved = if plus { 1 } else { -1 };
                }
            }
            if moved != 0 {
                direction = moved;
                source_side = side;
            }
        }
        if reverse {
            direction = -direction;
        }
        if direction != 0 && self.traces < 5 {
            self.traces += 1;
            #[cfg(all(windows, target_arch = "x86_64"))]
            crate::runtime::trace_mmd_scratch(source_side, direction);
            #[cfg(not(all(windows, target_arch = "x86_64")))]
            let _ = source_side;
        }
        direction
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn circular_motion_ignores_jitter_and_wraps_in_both_directions() {
        let mut a = Axis::default();
        assert_eq!(a.sample(Some(0.997)), 0);
        assert_eq!(a.sample(Some(0.999)), 0);
        assert_eq!(a.sample(Some(0.003)), 1);
        assert_eq!(a.sample(Some(0.995)), -1);
        assert_eq!(a.sample(Some(0.994)), 0);
        assert_eq!(a.sample(Some(0.99)), -1);
        assert_eq!(a.sample(Some(0.5)), 0); // discontinuity
        assert_eq!(a.sample(Some(f32::NAN)), 0);
        assert_eq!(a.sample(Some(0.1)), 0); // new baseline
        assert_eq!(a.sample(None), 0);
        assert_eq!(a.sample(Some(0.8)), 0);
    }

    #[test]
    fn sdk_reads_are_optional_read_only_and_direction_can_be_reversed() {
        static VALUE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        unsafe extern "C" fn analog(side: u32, out: *mut f32) -> i32 {
            unsafe {
                *out = if side == 0 {
                    f32::from_bits(VALUE.load(Ordering::Relaxed))
                } else {
                    0.
                };
            }
            0
        }
        let mut scratch = Scratch::default();
        configure(analog as *const () as usize, 0);
        VALUE.store(0.2f32.to_bits(), Ordering::Relaxed);
        assert_eq!(scratch.poll(false), 0);
        VALUE.store(0.25f32.to_bits(), Ordering::Relaxed);
        assert_eq!(scratch.poll(false), 1);
        VALUE.store(0.3f32.to_bits(), Ordering::Relaxed);
        assert_eq!(scratch.poll(true), -1);
        configure(0, 0);
        assert_eq!(scratch.poll(false), 0);
    }
}
