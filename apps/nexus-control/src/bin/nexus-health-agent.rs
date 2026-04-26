use anyhow::{Context, Result};

const ENV_TOKIO_WORKER_THREADS: &str = "TOKIO_WORKER_THREADS";
const DEFAULT_TOKIO_WORKER_THREADS: usize = 4;

fn main() -> Result<()> {
    let worker_threads = configured_tokio_worker_threads()?;
    build_runtime(worker_threads)?.block_on(async_main())
}

async fn async_main() -> Result<()> {
    ensure_rustls_crypto_provider()?;
    let args: Vec<String> = std::env::args().collect();
    let command = nexus_control::parse_nexus_health_agent_command(&args).map_err(|error| {
        anyhow::anyhow!(
            "failed to parse nexus-health-agent command: {error}\nusage: {}",
            nexus_control::nexus_health_agent_usage()
        )
    })?;
    let report = nexus_control::run_nexus_health_agent(&command).await?;
    let output = if command.pretty {
        serde_json::to_string_pretty(&report)
    } else {
        serde_json::to_string(&report)
    }
    .context("serialize nexus health-agent report")?;
    println!("{output}");
    Ok(())
}

fn build_runtime(worker_threads: usize) -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(worker_threads)
        .build()
        .context("failed to build nexus-health-agent tokio runtime")
}

fn ensure_rustls_crypto_provider() -> Result<()> {
    if rustls::crypto::CryptoProvider::get_default().is_some() {
        return Ok(());
    }

    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|error| anyhow::anyhow!("failed to install rustls crypto provider: {error:?}"))
}

fn configured_tokio_worker_threads() -> Result<usize> {
    match std::env::var(ENV_TOKIO_WORKER_THREADS) {
        Ok(value) => parse_tokio_worker_threads(Some(value.as_str())),
        Err(std::env::VarError::NotPresent) => Ok(DEFAULT_TOKIO_WORKER_THREADS),
        Err(std::env::VarError::NotUnicode(_)) => {
            anyhow::bail!("{ENV_TOKIO_WORKER_THREADS} must contain valid unicode")
        }
    }
}

fn parse_tokio_worker_threads(raw: Option<&str>) -> Result<usize> {
    let Some(raw) = raw.map(str::trim).filter(|raw| !raw.is_empty()) else {
        return Ok(DEFAULT_TOKIO_WORKER_THREADS);
    };
    let worker_threads = raw
        .parse::<usize>()
        .with_context(|| format!("invalid {ENV_TOKIO_WORKER_THREADS}: {raw}"))?;
    if worker_threads == 0 {
        anyhow::bail!("{ENV_TOKIO_WORKER_THREADS} must be greater than zero");
    }
    Ok(worker_threads)
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_TOKIO_WORKER_THREADS, parse_tokio_worker_threads};

    #[test]
    fn worker_threads_default_when_blank() {
        assert_eq!(
            parse_tokio_worker_threads(Some(" ")).expect("parse"),
            DEFAULT_TOKIO_WORKER_THREADS
        );
    }

    #[test]
    fn worker_threads_reject_zero() {
        assert!(parse_tokio_worker_threads(Some("0")).is_err());
    }
}
