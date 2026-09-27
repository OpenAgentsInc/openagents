use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct MonsterSpec {
    pub id: String,
    pub ac: i32,
    pub hp: i32,
    #[serde(default)]
    pub save_mods: std::collections::HashMap<String, i32>,
}
