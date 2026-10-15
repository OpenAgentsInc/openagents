//! Role-scoped credentials. Formatting never includes credential bytes.
use crate::{Error, Result};
use std::{fmt, path::Path};
use tonic::{metadata::MetadataValue, service::Interceptor};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Invoice,
    ReadOnly,
    Payout,
    Ops,
}
#[derive(Clone)]
pub struct Macaroon {
    role: Role,
    root_key_id: u64,
    bytes: Zeroizing<Vec<u8>>,
}
impl Macaroon {
    pub fn new(role: Role, root_key_id: u64, bytes: Vec<u8>) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > 64 * 1024 {
            return Err(Error::Invalid("macaroon"));
        }
        Ok(Self {
            role,
            root_key_id,
            bytes: Zeroizing::new(bytes),
        })
    }
    pub fn from_file(role: Role, root_key_id: u64, path: &Path) -> Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let meta = std::fs::symlink_metadata(path).map_err(|_| Error::PermissionDenied)?;
            if !meta.is_file() || meta.mode() & 0o077 != 0 {
                return Err(Error::PermissionDenied);
            }
        }
        Self::new(
            role,
            root_key_id,
            std::fs::read(path).map_err(|_| Error::PermissionDenied)?,
        )
    }
    pub fn role(&self) -> Role {
        self.role
    }
    /// Save a newly issued credential without exposing it or replacing another one.
    pub fn write_new(&self, path: &Path) -> Result<()> {
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path).map_err(|_| Error::PermissionDenied)?;
        file.write_all(&self.bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| Error::Checkpoint)
    }
}
impl fmt::Debug for Macaroon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "macaroon({:?}, {})", self.role, self.root_key_id)
    }
}
impl fmt::Display for Macaroon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
impl Interceptor for Macaroon {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> std::result::Result<tonic::Request<()>, tonic::Status> {
        let text = Zeroizing::new(hex::encode(&*self.bytes));
        let mut value = MetadataValue::try_from(text.as_str())
            .map_err(|_| tonic::Status::unauthenticated("invalid credential"))?;
        value.set_sensitive(true);
        request.metadata_mut().insert("macaroon", value);
        Ok(request)
    }
}

impl crate::Client {
    /// Bake a distinct root key for each service role. Production services
    /// load the result from tmpfs; only an operator uses this method.
    pub async fn bake_macaroon(&self, role: Role, root_key_id: u64) -> Result<Macaroon> {
        if root_key_id == 0 {
            return Err(Error::Invalid("macaroon root key"));
        }
        let grants: &[(&str, &str)] = match role {
            Role::Invoice => &[
                ("info", "read"),
                ("invoices", "read"),
                ("invoices", "write"),
                ("offchain", "read"),
            ],
            Role::ReadOnly => &[
                ("info", "read"),
                ("invoices", "read"),
                ("offchain", "read"),
                ("onchain", "read"),
                ("peers", "read"),
                ("address", "read"),
            ],
            Role::Payout => &[
                ("info", "read"),
                ("offchain", "read"),
                ("offchain", "write"),
            ],
            Role::Ops => &[
                ("info", "read"),
                ("offchain", "read"),
                ("offchain", "write"),
                ("onchain", "read"),
                ("onchain", "write"),
                ("peers", "read"),
                ("peers", "write"),
                ("address", "read"),
                ("address", "write"),
            ],
        };
        self.ensure_current().await?;
        let reply = crate::transport::once(
            self.rpc()
                .bake_macaroon(crate::proto::lnrpc::BakeMacaroonRequest {
                    root_key_id,
                    permissions: grants
                        .iter()
                        .map(|(entity, action)| crate::proto::lnrpc::MacaroonPermission {
                            entity: (*entity).into(),
                            action: (*action).into(),
                        })
                        .collect(),
                    ..Default::default()
                }),
        )
        .await?;
        let secret = Zeroizing::new(reply.macaroon);
        Macaroon::new(
            role,
            root_key_id,
            hex::decode(secret.as_str()).map_err(|_| Error::Invalid("macaroon"))?,
        )
    }
}
