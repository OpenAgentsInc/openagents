#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FocusId(u64);

impl FocusId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl From<u64> for FocusId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FocusHandle {
    id: FocusId,
}

impl FocusHandle {
    pub const fn new(id: u64) -> Self {
        Self {
            id: FocusId::new(id),
        }
    }

    pub const fn id(self) -> FocusId {
        self.id
    }

    pub const fn value(self) -> u64 {
        self.id.value()
    }
}

impl From<u64> for FocusHandle {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl From<FocusHandle> for u64 {
    fn from(handle: FocusHandle) -> Self {
        handle.value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_handle() {
        let handle = FocusHandle::new(42);
        assert_eq!(handle.value(), 42);
        assert_eq!(handle.id().value(), 42);
    }
}
