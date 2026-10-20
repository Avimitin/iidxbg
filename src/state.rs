//! CPU-only visit selection. A late result can never change an active visit.
use crate::pixels::Prepared;
use std::sync::Arc;

#[derive(Default)]
pub struct Selection {
    pub active: bool,
    pub selected: Option<Arc<Prepared>>,
}

impl Selection {
    pub fn enter(&mut self, pending: Option<Arc<Prepared>>) {
        if self.active {
            return;
        }
        self.active = true;
        if let Some(pending) = pending {
            self.selected = Some(pending);
        }
    }

    pub fn leave(&mut self) {
        self.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn picture(label: &str) -> Arc<Prepared> {
        Arc::new(Prepared {
            label: label.into(),
            width: 1,
            height: 1,
            bgra: vec![0; 4],
        })
    }
    #[test]
    fn late_completion_waits_and_failure_keeps_previous() {
        let mut s = Selection::default();
        s.enter(Some(picture("A")));
        s.enter(Some(picture("B")));
        assert_eq!(s.selected.as_ref().unwrap().label, "A");
        s.leave();
        s.enter(None);
        assert_eq!(s.selected.as_ref().unwrap().label, "A");
        s.leave();
        s.enter(Some(picture("B")));
        assert_eq!(s.selected.as_ref().unwrap().label, "B");
    }
    #[test]
    fn empty_visit_does_not_adopt_late_image() {
        let mut s = Selection::default();
        s.enter(None);
        s.enter(Some(picture("late")));
        assert!(s.selected.is_none());
    }
}
