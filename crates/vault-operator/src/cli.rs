//! Operator custody commands. The key service here is a local synthetic key
//! from an owner-only file; production mounts its KMS behind the same trait.
use crate::{
    Result,
    service::{Config, Service},
};
use oa_vault::operator::LocalOperatorKms;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use vault_sealed::Signed;

pub const USAGE: &str = "usage: vault-operator ACTION --dir DIR [options]
  init     --config CONFIG.json
  grant    --kms-key FILE --kms-version NAME --input SIGNED.json
  use      --kms-key FILE --kms-version NAME --input SIGNED.json --object FILE
  revoke   --kms-key FILE --kms-version NAME --input SIGNED.json
  audit    --kms-key FILE --kms-version NAME";

fn read_key(path: &Path) -> Result<[u8; 32]> {
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("not_admitted: the key file must be readable only by its owner".into());
        }
    }
    let _ = metadata;
    oa_vault::unhex::<32>(
        std::fs::read_to_string(path)
            .map_err(|e| e.to_string())?
            .trim(),
    )
    .map_err(|_| "malformed: key file".into())
}

pub fn run(args: Vec<String>) -> Result<Value> {
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        return Ok(json!({"help":USAGE}));
    }
    let need = |name: &str| -> Result<String> {
        let index = args
            .iter()
            .position(|a| a == name)
            .ok_or_else(|| format!("malformed: missing {name}"))?;
        args.get(index + 1)
            .cloned()
            .ok_or_else(|| format!("malformed: missing value for {name}"))
    };
    let dir = PathBuf::from(need("--dir")?);
    if args[0] == "init" {
        let config: Config =
            serde_json::from_slice(&std::fs::read(need("--config")?).map_err(|e| e.to_string())?)
                .map_err(|_| "malformed: config")?;
        Service::initialize(dir, config)?;
        return Ok(json!({"created":true}));
    }
    let kms = LocalOperatorKms::new(
        &need("--kms-version")?,
        read_key(Path::new(&need("--kms-key")?))?,
    );
    let mut service = Service::open(dir, Box::new(kms))?;
    let signed = || -> Result<Signed> {
        serde_json::from_slice(&std::fs::read(need("--input")?).map_err(|e| e.to_string())?)
            .map_err(|_| "malformed: input".into())
    };
    let now = crate::now();
    match args[0].as_str() {
        "grant" => Ok(json!({"grant":service.grant(&signed()?, now)?})),
        "revoke" => {
            service.revoke(&signed()?, now)?;
            Ok(json!({"revoked":true}))
        }
        "use" => {
            let bytes = std::fs::read(need("--object")?).map_err(|e| e.to_string())?;
            let outcome = service.use_object(&signed()?, &bytes, now)?;
            Ok(json!({"status":outcome.status,"reason":outcome.reason,
                "plaintext":outcome.plaintext.map(|p| oa_vault::b64(&p))}))
        }
        "audit" => Ok(json!({"entries":service.audit().verify()?})),
        _ => Err(format!("malformed: {USAGE}")),
    }
}
