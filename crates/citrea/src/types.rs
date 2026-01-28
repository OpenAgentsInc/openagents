use serde::Serialize;

pub type Address = [u8; 20];
pub type Bytes32 = [u8; 32];
pub type Bytes64 = [u8; 64];

#[derive(Clone, Copy, Debug)]
pub enum BlockTag {
    Latest,
    Pending,
    Earliest,
    Number(u64),
}

impl BlockTag {
    pub fn to_param(self) -> serde_json::Value {
        match self {
            BlockTag::Latest => serde_json::Value::String("latest".to_string()),
            BlockTag::Pending => serde_json::Value::String("pending".to_string()),
            BlockTag::Earliest => serde_json::Value::String("earliest".to_string()),
            BlockTag::Number(value) => serde_json::Value::String(format!("0x{value:x}")),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RpcCallRequest {
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_price: Option<String>,
}
