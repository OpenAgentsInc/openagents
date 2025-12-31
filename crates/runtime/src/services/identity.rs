//! Identity filesystem service.

use crate::fs::{
    BytesHandle, DirEntry, FileHandle, FileService, FsError, FsResult, OpenFlags, Stat,
};
use crate::identity::{PublicKey, Signature, SigningService};
use crate::types::AgentId;
use std::sync::Arc;

/// Identity service backed by a signing service.
#[derive(Clone)]
pub struct IdentityFs {
    signer: Arc<dyn SigningService>,
    agent_id: AgentId,
}

impl IdentityFs {
    /// Create an identity service.
    pub fn new(agent_id: AgentId, signer: Arc<dyn SigningService>) -> Self {
        Self { signer, agent_id }
    }
}

impl FileService for IdentityFs {
    fn open(&self, path: &str, _flags: OpenFlags) -> FsResult<Box<dyn FileHandle>> {
        match path {
            "pubkey" => {
                let pubkey = self
                    .signer
                    .pubkey(&self.agent_id)
                    .map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(pubkey.to_hex().into_bytes())))
            }
            "sign" => Ok(Box::new(SignHandle::new(
                self.signer.clone(),
                self.agent_id.clone(),
            ))),
            "verify" => Ok(Box::new(VerifyHandle::new(self.signer.clone()))),
            "encrypt" => Ok(Box::new(EncryptHandle::new(
                self.signer.clone(),
                self.agent_id.clone(),
            ))),
            "decrypt" => Ok(Box::new(DecryptHandle::new(
                self.signer.clone(),
                self.agent_id.clone(),
            ))),
            _ => Err(FsError::NotFound),
        }
    }

    fn readdir(&self, path: &str) -> FsResult<Vec<DirEntry>> {
        match path {
            "" => Ok(vec![
                DirEntry::file("pubkey", 0),
                DirEntry::file("sign", 0),
                DirEntry::file("verify", 0),
                DirEntry::file("encrypt", 0),
                DirEntry::file("decrypt", 0),
            ]),
            _ => Err(FsError::NotFound),
        }
    }

    fn stat(&self, path: &str) -> FsResult<Stat> {
        match path {
            "" => Ok(Stat::dir()),
            "pubkey" | "sign" | "verify" | "encrypt" | "decrypt" => Ok(Stat::file(0)),
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
        "identity"
    }
}

struct SignHandle {
    signer: Arc<dyn SigningService>,
    agent_id: AgentId,
    buffer: Vec<u8>,
    output: Option<Vec<u8>>,
    position: usize,
}

impl SignHandle {
    fn new(signer: Arc<dyn SigningService>, agent_id: AgentId) -> Self {
        Self {
            signer,
            agent_id,
            buffer: Vec::new(),
            output: None,
            position: 0,
        }
    }
}

impl FileHandle for SignHandle {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if self.output.is_none() {
            let sig = self
                .signer
                .sign(&self.agent_id, &self.buffer)
                .map_err(|err| FsError::Other(err.to_string()))?;
            self.output = Some(sig.to_hex().into_bytes());
        }

        let output = self.output.as_ref().expect("signature bytes");
        if self.position >= output.len() {
            return Ok(0);
        }
        let len = (output.len() - self.position).min(buf.len());
        buf[..len].copy_from_slice(&output[self.position..self.position + len]);
        self.position += len;
        Ok(len)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: crate::fs::SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
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

struct VerifyHandle {
    signer: Arc<dyn SigningService>,
    buffer: Vec<u8>,
    output: Option<Vec<u8>>,
    position: usize,
}

impl VerifyHandle {
    fn new(signer: Arc<dyn SigningService>) -> Self {
        Self {
            signer,
            buffer: Vec::new(),
            output: None,
            position: 0,
        }
    }
}

impl FileHandle for VerifyHandle {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if self.output.is_none() {
            let payload: serde_json::Value = serde_json::from_slice(&self.buffer)
                .map_err(|err| FsError::Other(err.to_string()))?;
            let pubkey_hex = payload
                .get("pubkey")
                .and_then(|value| value.as_str())
                .ok_or_else(|| FsError::Other("missing pubkey".into()))?;
            let signature_hex = payload
                .get("signature")
                .and_then(|value| value.as_str())
                .ok_or_else(|| FsError::Other("missing signature".into()))?;
            let message = payload
                .get("message")
                .and_then(|value| value.as_str())
                .ok_or_else(|| FsError::Other("missing message".into()))?;

            let pubkey_bytes =
                hex::decode(pubkey_hex).map_err(|err| FsError::Other(err.to_string()))?;
            let sig_bytes =
                hex::decode(signature_hex).map_err(|err| FsError::Other(err.to_string()))?;
            let pubkey = PublicKey::new(pubkey_bytes);
            let signature = Signature::new(sig_bytes);

            let result = self.signer.verify(&pubkey, message.as_bytes(), &signature);
            let output = if result { "true" } else { "false" };
            self.output = Some(output.as_bytes().to_vec());
        }

        let output = self.output.as_ref().expect("verify output");
        if self.position >= output.len() {
            return Ok(0);
        }
        let len = (output.len() - self.position).min(buf.len());
        buf[..len].copy_from_slice(&output[self.position..self.position + len]);
        self.position += len;
        Ok(len)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: crate::fs::SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
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

struct EncryptHandle {
    signer: Arc<dyn SigningService>,
    agent_id: AgentId,
    buffer: Vec<u8>,
    output: Option<Vec<u8>>,
    position: usize,
}

impl EncryptHandle {
    fn new(signer: Arc<dyn SigningService>, agent_id: AgentId) -> Self {
        Self {
            signer,
            agent_id,
            buffer: Vec::new(),
            output: None,
            position: 0,
        }
    }
}

impl FileHandle for EncryptHandle {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if self.output.is_none() {
            let payload: serde_json::Value = serde_json::from_slice(&self.buffer)
                .map_err(|err| FsError::Other(err.to_string()))?;
            let recipient_hex = payload
                .get("recipient")
                .and_then(|value| value.as_str())
                .ok_or_else(|| FsError::Other("missing recipient".into()))?;
            let message = payload
                .get("message")
                .and_then(|value| value.as_str())
                .ok_or_else(|| FsError::Other("missing message".into()))?;

            let recipient_bytes =
                hex::decode(recipient_hex).map_err(|err| FsError::Other(err.to_string()))?;
            let recipient = PublicKey::new(recipient_bytes);

            let encrypted = self
                .signer
                .encrypt(&self.agent_id, &recipient, message.as_bytes())
                .map_err(|err| FsError::Other(err.to_string()))?;
            self.output = Some(encrypted);
        }

        let output = self.output.as_ref().expect("encrypt output");
        if self.position >= output.len() {
            return Ok(0);
        }
        let len = (output.len() - self.position).min(buf.len());
        buf[..len].copy_from_slice(&output[self.position..self.position + len]);
        self.position += len;
        Ok(len)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: crate::fs::SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
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

struct DecryptHandle {
    signer: Arc<dyn SigningService>,
    agent_id: AgentId,
    buffer: Vec<u8>,
    output: Option<Vec<u8>>,
    position: usize,
}

impl DecryptHandle {
    fn new(signer: Arc<dyn SigningService>, agent_id: AgentId) -> Self {
        Self {
            signer,
            agent_id,
            buffer: Vec::new(),
            output: None,
            position: 0,
        }
    }
}

impl FileHandle for DecryptHandle {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if self.output.is_none() {
            let payload: serde_json::Value = serde_json::from_slice(&self.buffer)
                .map_err(|err| FsError::Other(err.to_string()))?;
            let sender_hex = payload
                .get("sender")
                .and_then(|value| value.as_str())
                .ok_or_else(|| FsError::Other("missing sender".into()))?;
            let ciphertext_hex = payload
                .get("ciphertext")
                .and_then(|value| value.as_str())
                .ok_or_else(|| FsError::Other("missing ciphertext".into()))?;

            let sender_bytes =
                hex::decode(sender_hex).map_err(|err| FsError::Other(err.to_string()))?;
            let sender = PublicKey::new(sender_bytes);
            let ciphertext =
                hex::decode(ciphertext_hex).map_err(|err| FsError::Other(err.to_string()))?;

            let decrypted = self
                .signer
                .decrypt(&self.agent_id, &sender, &ciphertext)
                .map_err(|err| FsError::Other(err.to_string()))?;
            self.output = Some(decrypted);
        }

        let output = self.output.as_ref().expect("decrypt output");
        if self.position >= output.len() {
            return Ok(0);
        }
        let len = (output.len() - self.position).min(buf.len());
        buf[..len].copy_from_slice(&output[self.position..self.position + len]);
        self.position += len;
        Ok(len)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: crate::fs::SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
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
