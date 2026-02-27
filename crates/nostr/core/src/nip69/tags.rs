use super::{BitcoinLayer, DOCUMENT_TYPE, OrderStatus, OrderType};

/// Create order ID tag
pub fn create_order_id_tag(order_id: String) -> Vec<String> {
    vec!["d".to_string(), order_id]
}

/// Create order type tag
pub fn create_order_type_tag(order_type: OrderType) -> Vec<String> {
    vec!["k".to_string(), order_type.as_str().to_string()]
}

/// Create currency tag (ISO 4217)
pub fn create_currency_tag(currency: String) -> Vec<String> {
    vec!["f".to_string(), currency]
}

/// Create status tag
pub fn create_status_tag(status: OrderStatus) -> Vec<String> {
    vec!["s".to_string(), status.as_str().to_string()]
}

/// Create amount tag
pub fn create_amount_tag(amount_sats: u64) -> Vec<String> {
    vec!["amt".to_string(), amount_sats.to_string()]
}

/// Create fiat amount tag
pub fn create_fiat_amount_tag(amounts: Vec<u64>) -> Vec<String> {
    let mut tag = vec!["fa".to_string()];
    tag.extend(amounts.iter().map(|a| a.to_string()));
    tag
}

/// Create payment method tag
pub fn create_payment_method_tag(methods: Vec<String>) -> Vec<String> {
    let mut tag = vec!["pm".to_string()];
    tag.extend(methods);
    tag
}

/// Create layer tag
pub fn create_layer_tag(layer: BitcoinLayer) -> Vec<String> {
    vec!["layer".to_string(), layer.as_str().to_string()]
}

/// Create platform tag
pub fn create_platform_tag(platform: String) -> Vec<String> {
    vec!["y".to_string(), platform]
}

/// Create document type tag
pub fn create_document_tag() -> Vec<String> {
    vec!["z".to_string(), DOCUMENT_TYPE.to_string()]
}
