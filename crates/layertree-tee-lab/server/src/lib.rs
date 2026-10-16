//! Bounded HTTP and local Bitcoin Core access for signet research.
use layertree_tee_lab::rpc::MAX_BODY;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    time::Duration,
};

pub struct Request {
    pub method: String,
    pub path: String,
    pub body: Vec<u8>,
}

/// One request per connection, a fixed header limit, and no chunked bodies.
pub fn request(stream: &mut impl Read) -> Result<Request, String> {
    let mut raw = Vec::new();
    let header_end = loop {
        if raw.len() >= 8192 {
            return Err("request headers are too large".into());
        }
        let mut byte = [0];
        stream
            .read_exact(&mut byte)
            .map_err(|_| "cannot read request")?;
        raw.push(byte[0]);
        if raw.ends_with(b"\r\n\r\n") {
            break raw.len();
        }
    };
    let mut headers = [httparse::EMPTY_HEADER; 32];
    let mut parsed = httparse::Request::new(&mut headers);
    if !parsed
        .parse(&raw)
        .map_err(|_| "invalid HTTP request")?
        .is_complete()
    {
        return Err("invalid HTTP request".into());
    }
    let method = parsed.method.ok_or("missing method")?.to_owned();
    let path = parsed.path.ok_or("missing path")?.to_owned();
    let mut length = None;
    for header in parsed.headers.iter() {
        if header.name.eq_ignore_ascii_case("transfer-encoding") {
            return Err("chunked requests are refused".into());
        }
        if header.name.eq_ignore_ascii_case("content-length") {
            if length.is_some() {
                return Err("duplicate content length".into());
            }
            length = Some(
                std::str::from_utf8(header.value)
                    .map_err(|_| "invalid content length")?
                    .parse::<usize>()
                    .map_err(|_| "invalid content length")?,
            );
        }
    }
    let length = length.unwrap_or(0);
    if length > MAX_BODY || (method != "POST" && length != 0) {
        return Err("invalid body length".into());
    }
    let mut body = vec![0; length];
    stream
        .read_exact(&mut body)
        .map_err(|_| "truncated request")?;
    debug_assert_eq!(header_end, raw.len());
    Ok(Request { method, path, body })
}

pub fn response(stream: &mut impl Write, status: u16, body: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(body).map_err(|_| "cannot encode response")?;
    response_bytes(stream, status, "application/json", &bytes)
}

pub fn response_bytes(
    stream: &mut impl Write,
    status: u16,
    content_type: &str,
    bytes: &[u8],
) -> Result<(), String> {
    write!(stream,"HTTP/1.1 {status} Response\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n",bytes.len()).map_err(|_|"cannot write response")?;
    stream
        .write_all(&bytes)
        .map_err(|_| "cannot write response")?;
    stream.flush().map_err(|_| "cannot flush response".into())
}

pub fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(45))
        .build()
        .map_err(|_| "cannot create HTTP client".into())
}
pub fn bounded_json(response: reqwest::blocking::Response) -> Result<Value, String> {
    if !response.status().is_success() {
        return Err(format!("remote request refused ({})", response.status()));
    }
    let mut bytes = Vec::new();
    response
        .take((MAX_BODY + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read remote response")?;
    if bytes.len() > MAX_BODY {
        return Err("remote response is too large".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "invalid remote JSON".into())
}

pub struct Core {
    client: reqwest::blocking::Client,
    cookie: std::path::PathBuf,
}
impl Core {
    pub fn new(cookie: impl Into<std::path::PathBuf>) -> Result<Self, String> {
        Ok(Self {
            client: client()?,
            cookie: cookie.into(),
        })
    }
    pub fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        // This credential never leaves the guest's loopback connection.
        let cookie = std::fs::read_to_string(&self.cookie).map_err(|_| "Bitcoin is starting")?;
        let (user, password) = cookie
            .trim()
            .split_once(':')
            .ok_or("invalid local Bitcoin cookie")?;
        let result = bounded_json(
            self.client
                .post("http://127.0.0.1:38332/")
                .basic_auth(user, Some(password))
                .json(&json!({"jsonrpc":"2.0","id":"lt","method":method,"params":params}))
                .send()
                .map_err(|_| "Bitcoin is unavailable")?,
        )?;
        if !result["error"].is_null() {
            return Err("Bitcoin refused the request".into());
        }
        Ok(result["result"].clone())
    }
    pub fn ready_height(&self) -> Result<u32, String> {
        let info = self.call("getblockchaininfo", json!([]))?;
        if info["chain"] != "signet" || info["initialblockdownload"] != false {
            return Err("Bitcoin is syncing signet".into());
        }
        u32::try_from(info["blocks"].as_u64().ok_or("invalid Bitcoin height")?)
            .map_err(|_| "invalid Bitcoin height".into())
    }
    pub fn output(
        &self,
        point: bitcoin::OutPoint,
        confirmed: bool,
    ) -> Result<bitcoin::TxOut, String> {
        let value = self.call(
            "gettxout",
            json!([point.txid.to_string(), point.vout, true]),
        )?;
        if value.is_null() || (confirmed && value["confirmations"].as_u64().unwrap_or(0) == 0) {
            return Err("input is missing or unconfirmed".into());
        }
        let amount = value["value"]
            .to_string()
            .parse::<bitcoin::Amount>()
            .or_else(|_| {
                bitcoin::Amount::from_str_in(
                    &value["value"].to_string(),
                    bitcoin::Denomination::Bitcoin,
                )
            })
            .map_err(|_| "invalid Bitcoin amount")?;
        let script = bitcoin::ScriptBuf::from_bytes(layertree_tee_lab::rpc::bytes(
            value["scriptPubKey"]["hex"]
                .as_str()
                .ok_or("missing Bitcoin script")?,
        )?);
        Ok(bitcoin::TxOut {
            value: amount,
            script_pubkey: script,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn framing_refuses_smuggling_and_oversize() {
        for raw in [
            "POST / HTTP/1.1\r\nContent-Length: 0\r\nContent-Length: 0\r\n\r\n",
            "POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n",
            "POST / HTTP/1.1\r\nContent-Length: 1048577\r\n\r\n",
            "GET / HTTP/1.1\r\nContent-Length: 1\r\n\r\nx",
        ] {
            assert!(request(&mut std::io::Cursor::new(raw.as_bytes())).is_err());
        }
        let req = request(&mut std::io::Cursor::new(
            b"POST /next HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}".to_vec(),
        ))
        .unwrap();
        assert_eq!(req.path, "/next");
        assert_eq!(req.body, b"{}");
    }
}
