//! CPU-only visit selection. A late result can never change an active visit.
use crate::pixels::Prepared;
use std::sync::Arc;

pub enum Pending {
    Image(Arc<Prepared>),
    Video(Arc<crate::video::Video>),
    Disabled,
}

#[derive(Default)]
pub struct Selection {
    pub active: bool,
    pub selected: Option<Arc<Prepared>>,
    pub video: Option<Arc<crate::video::Video>>,
}

impl Selection {
    pub fn enter(&mut self, pending: Option<Pending>) {
        if self.active {
            return;
        }
        self.active = true;
        match pending {
            Some(Pending::Image(image)) => {
                self.selected = Some(image);
                self.video = None;
            }
            Some(Pending::Video(video)) => {
                self.selected = Some(video.first.clone());
                self.video = Some(video);
            }
            Some(Pending::Disabled) => {
                self.selected = None;
                self.video = None;
            }
            None => {}
        }
    }

    pub fn leave(&mut self) {
        self.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn picture(label: &str) -> Pending {
        Pending::Image(Arc::new(Prepared {
            label: label.into(),
            width: 1,
            height: 1,
            bgra: vec![0; 4],
        }))
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

    #[test]
    fn video_choice_is_frozen_until_next_visit_and_disable_clears_it() {
        let Pending::Image(first) = picture("first frame") else {
            unreachable!()
        };
        let video = Arc::new(crate::video::Video::new(first));
        let mut s = Selection::default();
        s.enter(Some(picture("image")));
        s.enter(Some(Pending::Video(video.clone())));
        assert!(s.video.is_none());
        s.leave();
        s.enter(Some(Pending::Video(video.clone())));
        assert!(Arc::ptr_eq(s.video.as_ref().unwrap(), &video));
        s.enter(Some(Pending::Disabled));
        assert!(s.video.is_some());
        s.leave();
        s.enter(Some(Pending::Disabled));
        assert!(s.video.is_none());
        assert!(s.selected.is_none());
    }

    #[test]
    fn disabling_waits_for_next_visit_and_can_be_reenabled() {
        let mut s = Selection::default();
        s.enter(Some(picture("A")));
        s.enter(Some(Pending::Disabled));
        assert_eq!(s.selected.as_ref().unwrap().label, "A");
        s.leave();
        s.enter(Some(Pending::Disabled));
        assert!(s.selected.is_none());
        s.leave();
        s.enter(None);
        assert!(s.selected.is_none());
        s.enter(Some(picture("B")));
        assert!(s.selected.is_none());
        s.leave();
        s.enter(Some(picture("B")));
        assert_eq!(s.selected.as_ref().unwrap().label, "B");
    }
}
