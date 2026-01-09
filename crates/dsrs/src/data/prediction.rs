use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, ops::Index};

use crate::LmUsage;

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Prediction {
    pub data: HashMap<String, serde_json::Value>,
    pub lm_usage: LmUsage,
    #[serde(skip)]
    pub node_id: Option<usize>,
}

impl Prediction {
    pub fn new(data: HashMap<String, serde_json::Value>, lm_usage: LmUsage) -> Self {
        Self {
            data,
            lm_usage,
            node_id: None,
        }
    }

    pub fn get(&self, key: &str, default: Option<&str>) -> serde_json::Value {
        self.data
            .get(key)
            .unwrap_or(&default.unwrap_or_default().to_string().into())
            .clone()
    }

    pub fn get_tracked(&self, key: &str) -> crate::trace::TrackedValue {
        let val = self.get(key, None);
        crate::trace::TrackedValue {
            value: val,
            source: self.node_id.map(|id| (id, key.to_string())),
        }
    }

    pub fn keys(&self) -> Vec<String> {
        self.data.keys().cloned().collect()
    }

    pub fn values(&self) -> Vec<serde_json::Value> {
        self.data.values().cloned().collect()
    }

    pub fn set_lm_usage(&mut self, lm_usage: LmUsage) -> Self {
        self.lm_usage = lm_usage;
        self.clone()
    }
}

impl Index<String> for Prediction {
    type Output = serde_json::Value;

    fn index(&self, index: String) -> &Self::Output {
        &self.data[&index]
    }
}

impl Index<&str> for Prediction {
    type Output = serde_json::Value;

    fn index(&self, index: &str) -> &Self::Output {
        &self.data[index]
    }
}

impl IntoIterator for Prediction {
    type Item = (String, Value);
    type IntoIter = std::collections::hash_map::IntoIter<String, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

impl From<Vec<(String, Value)>> for Prediction {
    fn from(value: Vec<(String, Value)>) -> Self {
        Self {
            data: value.into_iter().collect(),
            lm_usage: LmUsage::default(),
            node_id: None,
        }
    }
}
