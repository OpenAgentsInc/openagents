//! Wallet filesystem service.

use crate::fs::{
    BytesHandle, DirEntry, FileHandle, FileService, FsError, FsResult, OpenFlags, SeekFrom, Stat,
};
use crate::fx::FxRateSnapshot;
use crate::types::Timestamp;
use crate::wallet::WalletService;
use serde_json::Value;
use std::str;
use std::sync::Arc;

/// Wallet filesystem service.
#[derive(Clone)]
pub struct WalletFs {
    wallet: Arc<dyn WalletService>,
}

impl WalletFs {
    /// Create a new wallet filesystem.
    pub fn new(wallet: Arc<dyn WalletService>) -> Self {
        Self { wallet }
    }

    fn balance_json(&self) -> FsResult<Vec<u8>> {
        let balance_sats = self
            .wallet
            .balance_sats()
            .map_err(|err| FsError::Other(err.to_string()))?;
        let fx = self.wallet.fx_rate().ok();
        let (balance_usd, fx_snapshot) = match fx {
            Some(snapshot) => {
                let balance_usd = if snapshot.sats_per_usd == 0 {
                    None
                } else {
                    let micro_usd = (u128::from(balance_sats) * 1_000_000u128)
                        / u128::from(snapshot.sats_per_usd);
                    Some(micro_usd as u64)
                };
                (balance_usd, Some(snapshot))
            }
            None => (None, None),
        };
        let json = serde_json::json!({
            "balance_sats": balance_sats,
            "balance_usd": balance_usd,
            "fx": fx_snapshot.map(|rate| fx_json(&rate)),
        });
        serde_json::to_vec(&json).map_err(|err| FsError::Other(err.to_string()))
    }

    fn fx_json(&self) -> FsResult<Vec<u8>> {
        let rate = self
            .wallet
            .fx_rate()
            .map_err(|err| FsError::Other(err.to_string()))?;
        let json = serde_json::json!(fx_json(&rate));
        serde_json::to_vec(&json).map_err(|err| FsError::Other(err.to_string()))
    }
}

impl FileService for WalletFs {
    fn open(&self, path: &str, flags: OpenFlags) -> FsResult<Box<dyn FileHandle>> {
        match path {
            "" => Err(FsError::IsDirectory),
            "balance" => Ok(Box::new(BytesHandle::new(self.balance_json()?))),
            "fx" => Ok(Box::new(BytesHandle::new(self.fx_json()?))),
            "pay" => {
                if !flags.write {
                    return Err(FsError::PermissionDenied);
                }
                Ok(Box::new(WalletPayHandle::new(self.wallet.clone())))
            }
            _ => Err(FsError::NotFound),
        }
    }

    fn readdir(&self, path: &str) -> FsResult<Vec<DirEntry>> {
        match path {
            "" => Ok(vec![
                DirEntry::file("balance", self.balance_json()?.len() as u64),
                DirEntry::file("fx", self.fx_json()?.len() as u64),
                DirEntry::file("pay", 0),
            ]),
            _ => Err(FsError::NotFound),
        }
    }

    fn stat(&self, path: &str) -> FsResult<Stat> {
        match path {
            "" => Ok(Stat::dir()),
            "balance" => Ok(Stat::file(self.balance_json()?.len() as u64)),
            "fx" => Ok(Stat::file(self.fx_json()?.len() as u64)),
            "pay" => Ok(Stat::file(0)),
            _ => Err(FsError::NotFound),
        }
    }

    fn mkdir(&self, _path: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn remove(&self, _path: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn rename(&self, _from: &str, _to: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn watch(&self, _path: &str) -> FsResult<Option<Box<dyn crate::fs::WatchHandle>>> {
        Ok(None)
    }

    fn name(&self) -> &str {
        "wallet"
    }
}

struct WalletPayHandle {
    wallet: Arc<dyn WalletService>,
    request_buf: Vec<u8>,
    response: Option<Vec<u8>>,
    position: usize,
}

impl WalletPayHandle {
    fn new(wallet: Arc<dyn WalletService>) -> Self {
        Self {
            wallet,
            request_buf: Vec::new(),
            response: None,
            position: 0,
        }
    }

    fn submit(&mut self) -> FsResult<()> {
        let input = str::from_utf8(&self.request_buf)
            .map_err(|err| FsError::Other(err.to_string()))?
            .trim()
            .to_string();
        if input.is_empty() {
            return Err(FsError::Other("invoice required".to_string()));
        }

        let (invoice, amount_sats) = parse_payment_request(&input)?;
        let payment = self
            .wallet
            .pay_invoice(&invoice, amount_sats)
            .map_err(|err| FsError::Other(err.to_string()))?;
        let json = serde_json::json!({
            "payment_id": payment.payment_id,
            "amount_sats": payment.amount_sats,
            "paid_at": Timestamp::now().as_millis(),
        });
        self.response =
            Some(serde_json::to_vec(&json).map_err(|err| FsError::Other(err.to_string()))?);
        Ok(())
    }
}

impl FileHandle for WalletPayHandle {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if self.response.is_none() {
            self.submit()?;
        }
        let response = self.response.as_ref().unwrap();
        if self.position >= response.len() {
            return Ok(0);
        }
        let len = std::cmp::min(buf.len(), response.len() - self.position);
        buf[..len].copy_from_slice(&response[self.position..self.position + len]);
        self.position += len;
        Ok(len)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.request_buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, pos: SeekFrom) -> FsResult<u64> {
        if self.response.is_none() {
            return Err(FsError::InvalidPath);
        }
        let response = self.response.as_ref().unwrap();
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => response.len() as i64 + offset,
            SeekFrom::Current(offset) => self.position as i64 + offset,
        };
        if new_pos < 0 {
            return Err(FsError::InvalidPath);
        }
        self.position = new_pos as usize;
        Ok(self.position as u64)
    }

    fn position(&self) -> u64 {
        self.position as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        Ok(())
    }
}

fn parse_payment_request(input: &str) -> FsResult<(String, Option<u64>)> {
    if input.starts_with('{') {
        let value: Value =
            serde_json::from_str(input).map_err(|err| FsError::Other(err.to_string()))?;
        let invoice = value
            .get("invoice")
            .and_then(|v| v.as_str())
            .ok_or_else(|| FsError::Other("invoice missing".to_string()))?
            .to_string();
        let amount_sats = value
            .get("amount_sats")
            .and_then(|v| v.as_u64())
            .or_else(|| {
                value
                    .get("amount_msats")
                    .and_then(|v| v.as_u64())
                    .map(|msats| (msats + 999) / 1000)
            });
        Ok((invoice, amount_sats))
    } else {
        Ok((input.to_string(), None))
    }
}

fn fx_json(rate: &FxRateSnapshot) -> Value {
    serde_json::json!({
        "sats_per_usd": rate.sats_per_usd,
        "updated_at": rate.updated_at.as_millis(),
    })
}
