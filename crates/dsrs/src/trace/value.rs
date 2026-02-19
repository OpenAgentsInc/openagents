use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize)]
pub struct TrackedValue {
    pub value: Value,
    #[serde(skip)]
    pub source: Option<(usize, String)>, // (node_id, key)
}

impl TrackedValue {
    pub fn new(value: Value, source: Option<(usize, String)>) -> Self {
        Self { value, source }
    }
}

pub trait IntoTracked {
    fn into_tracked(self) -> TrackedValue;
}

impl IntoTracked for TrackedValue {
    fn into_tracked(self) -> Self {
        self
    }
}

impl<T: Into<Value>> IntoTracked for T {
    fn into_tracked(self) -> TrackedValue {
        TrackedValue {
            value: self.into(),
            source: None,
        }
    }
}
