use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ClassSpec {
    pub id: String,
    pub base_ac: i32,
    pub spell_attack_bonus: i32,
    pub spell_save_dc: i32,
    #[serde(default)]
    pub save_mods: std::collections::HashMap<String, i32>,
}
