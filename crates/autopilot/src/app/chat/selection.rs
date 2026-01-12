use super::super::selection_point_cmp;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ChatSelectionPoint {
    pub(crate) message_index: usize,
    pub(crate) offset: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ChatSelection {
    pub(crate) anchor: ChatSelectionPoint,
    pub(crate) focus: ChatSelectionPoint,
}

impl ChatSelection {
    pub(crate) fn is_empty(&self) -> bool {
        self.anchor.message_index == self.focus.message_index
            && self.anchor.offset == self.focus.offset
    }

    pub(crate) fn normalized(&self) -> (ChatSelectionPoint, ChatSelectionPoint) {
        if selection_point_cmp(&self.anchor, &self.focus).is_gt() {
            (self.focus, self.anchor)
        } else {
            (self.anchor, self.focus)
        }
    }
}
