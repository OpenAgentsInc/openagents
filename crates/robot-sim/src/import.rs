//! Import adapter shared with Robot Lab's document store.
use crate::Result;
use robot_lab::{Store, document::Robot};
use std::path::Path;

pub fn pinned_urdf(store: &Store, root: &Path) -> Result<Robot> {
    robot_lab::import::pinned_urdf(store, root).map_err(Into::into)
}
