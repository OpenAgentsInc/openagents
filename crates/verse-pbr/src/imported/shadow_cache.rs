//! Exact-pose admission for grounded shadow casters.
pub(super) struct Frozen<K> {
    previous: Option<(K, Box<[u8]>)>,
    revision: u64,
}
impl<K> Default for Frozen<K> {
    fn default() -> Self {
        Self {
            previous: None,
            revision: 0,
        }
    }
}
impl<K: PartialEq> Frozen<K> {
    /// Two identical eligible samples admit a caster. Any identity or byte change
    /// removes it immediately; the next admission has a distinct revision.
    pub fn update(
        &mut self,
        identity: K,
        bytes: &[u8],
        eligible: bool,
    ) -> Result<Option<u64>, String> {
        if !eligible {
            self.previous = None;
            return Ok(None);
        }
        if self
            .previous
            .as_ref()
            .is_some_and(|(old, pose)| *old == identity && pose.as_ref() == bytes)
        {
            return Ok(Some(self.revision));
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or("Shadow pose revision exhausted")?;
        self.previous = Some((identity, bytes.into()));
        self.revision = revision;
        Ok(None)
    }
}
#[derive(Clone, PartialEq)]
pub(super) struct Face {
    pub matrix: [[f32; 4]; 4],
    pub casters: Vec<(usize, u64)>,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pose_identity_and_eligibility_changes_remove_cached_casters() {
        let mut state = Frozen::default();
        assert_eq!(state.update((1, 0), &[1, 2], true).unwrap(), None);
        let first = state.update((1, 0), &[1, 2], true).unwrap().unwrap();
        assert_eq!(state.update((1, 0), &[1, 2], true).unwrap(), Some(first));
        assert_eq!(state.update((1, 0), &[1, 3], true).unwrap(), None);
        let moved = state.update((1, 0), &[1, 3], true).unwrap().unwrap();
        assert_ne!(first, moved);
        assert_eq!(state.update((1, 1), &[1, 3], true).unwrap(), None);
        let respawned = state.update((1, 1), &[1, 3], true).unwrap().unwrap();
        assert_ne!(moved, respawned);
        assert_eq!(state.update((1, 1), &[1, 3], false).unwrap(), None);
        assert_eq!(state.update((1, 1), &[1, 3], true).unwrap(), None);
        assert_ne!(
            state.update((1, 1), &[1, 3], true).unwrap(),
            Some(respawned)
        );
    }
    #[test]
    fn exhaustion_refuses_without_relabeling_previous_pose() {
        let mut state = Frozen {
            previous: Some((1, vec![1].into())),
            revision: u64::MAX,
        };
        assert!(state.update(2, &[2], true).is_err());
        assert_eq!(state.update(1, &[1], true).unwrap(), Some(u64::MAX));
    }
    #[test]
    fn face_keys_track_light_pose_caster_revision_and_removal() {
        let first = Face {
            matrix: [[0.; 4]; 4],
            casters: vec![(1, 2)],
        };
        let mut changed = first.clone();
        changed.matrix[0][0] = 1.;
        assert!(first != changed);
        changed = first.clone();
        changed.casters[0].1 += 1;
        assert!(first != changed);
        changed = first.clone();
        changed.casters.clear();
        assert!(first != changed);
    }
}
