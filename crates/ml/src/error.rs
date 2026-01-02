use thiserror::Error;

pub type Result<T> = std::result::Result<T, MlError>;

#[derive(Error, Debug)]
pub enum MlError {
    #[error("device error: {0}")]
    Device(String),

    #[error("model error: {0}")]
    Model(String),

    #[error("tokenizer error: {0}")]
    Tokenizer(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("invalid config: {0}")]
    InvalidConfig(String),

    #[error("nostr error: {0}")]
    Nostr(String),
}

#[cfg(feature = "candle")]
impl From<candle_core::Error> for MlError {
    fn from(err: candle_core::Error) -> Self {
        MlError::Model(err.to_string())
    }
}

#[cfg(feature = "candle")]
impl From<tokenizers::Error> for MlError {
    fn from(err: tokenizers::Error) -> Self {
        MlError::Tokenizer(err.to_string())
    }
}

impl From<serde_json::Error> for MlError {
    fn from(err: serde_json::Error) -> Self {
        MlError::Serialization(err.to_string())
    }
}

impl From<std::io::Error> for MlError {
    fn from(err: std::io::Error) -> Self {
        MlError::Network(err.to_string())
    }
}

#[cfg(feature = "native")]
impl From<reqwest::Error> for MlError {
    fn from(err: reqwest::Error) -> Self {
        MlError::Network(err.to_string())
    }
}

#[cfg(feature = "browser")]
impl From<wasm_bindgen::JsValue> for MlError {
    fn from(err: wasm_bindgen::JsValue) -> Self {
        MlError::Network(format!("{err:?}"))
    }
}

#[cfg(feature = "browser")]
impl From<nostr::Nip01Error> for MlError {
    fn from(err: nostr::Nip01Error) -> Self {
        MlError::Nostr(err.to_string())
    }
}

#[cfg(feature = "browser")]
impl From<nostr::Nip19Error> for MlError {
    fn from(err: nostr::Nip19Error) -> Self {
        MlError::Nostr(err.to_string())
    }
}
