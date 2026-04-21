use anyhow::{Context, Result, bail};

const ENV_TOKIO_WORKER_THREADS: &str = "TOKIO_WORKER_THREADS";
const DEFAULT_SERVER_TOKIO_WORKER_THREADS: usize = 16;
const DEFAULT_LOG_FILTER: &str = "warn,nexus_relay=info,nexus_control=info";

fn main() -> Result<()> {
    let worker_threads = configured_tokio_worker_threads()?;
    build_runtime(worker_threads)?.block_on(async_main(worker_threads))
}

async fn async_main(worker_threads: usize) -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(DEFAULT_LOG_FILTER)),
        )
        .with_target(false)
        .compact()
        .init();
    tracing::info!(
        tokio_worker_threads = worker_threads,
        "starting nexus-relay runtime"
    );

    let config = nexus_relay::durable::DurableRelayConfig::from_env()
        .map_err(|error| anyhow::anyhow!("failed to load nexus-relay config: {error}"))?;
    nexus_relay::durable::run_server(config).await
}

fn build_runtime(worker_threads: usize) -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(worker_threads)
        .build()
        .context("failed to build nexus-relay tokio runtime")
}

fn configured_tokio_worker_threads() -> Result<usize> {
    match std::env::var(ENV_TOKIO_WORKER_THREADS) {
        Ok(value) => parse_tokio_worker_threads(Some(value.as_str())),
        Err(std::env::VarError::NotPresent) => Ok(DEFAULT_SERVER_TOKIO_WORKER_THREADS),
        Err(std::env::VarError::NotUnicode(_)) => {
            bail!("{ENV_TOKIO_WORKER_THREADS} must contain valid unicode")
        }
    }
}

fn parse_tokio_worker_threads(raw: Option<&str>) -> Result<usize> {
    let Some(raw) = raw.map(str::trim).filter(|raw| !raw.is_empty()) else {
        return Ok(DEFAULT_SERVER_TOKIO_WORKER_THREADS);
    };
    let worker_threads = raw
        .parse::<usize>()
        .with_context(|| format!("invalid {ENV_TOKIO_WORKER_THREADS}: {raw}"))?;
    if worker_threads == 0 {
        bail!("{ENV_TOKIO_WORKER_THREADS} must be greater than zero");
    }
    Ok(worker_threads)
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_SERVER_TOKIO_WORKER_THREADS, ENV_TOKIO_WORKER_THREADS, parse_tokio_worker_threads,
    };

    #[test]
    fn tokio_worker_threads_defaults_when_missing() {
        let parsed = parse_tokio_worker_threads(None).expect("default worker threads");
        assert_eq!(parsed, DEFAULT_SERVER_TOKIO_WORKER_THREADS);
    }

    #[test]
    fn tokio_worker_threads_defaults_when_blank() {
        let parsed = parse_tokio_worker_threads(Some("   ")).expect("blank defaults");
        assert_eq!(parsed, DEFAULT_SERVER_TOKIO_WORKER_THREADS);
    }

    #[test]
    fn tokio_worker_threads_accepts_positive_values() {
        let parsed = parse_tokio_worker_threads(Some("24")).expect("positive worker threads");
        assert_eq!(parsed, 24);
    }

    #[test]
    fn tokio_worker_threads_rejects_zero() {
        let error = parse_tokio_worker_threads(Some("0")).expect_err("zero should fail");
        assert!(
            error.to_string().contains(ENV_TOKIO_WORKER_THREADS),
            "expected env name in error, got: {error}"
        );
    }

    #[test]
    fn tokio_worker_threads_rejects_invalid_values() {
        let error = parse_tokio_worker_threads(Some("abc")).expect_err("invalid should fail");
        assert!(
            error.to_string().contains(ENV_TOKIO_WORKER_THREADS),
            "expected env name in error, got: {error}"
        );
    }
}
