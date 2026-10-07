use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;

use psionic_eval::{
    LegalBenchmarkSweepConfig, MockLegalBenchmarkSweepExecutor, run_legal_benchmark_sweep,
};

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().collect::<Vec<_>>();
    let config_path = args.get(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: legal_benchmark_sweep <sweep_config.json> <manifest.json>",
        )
    })?;
    let manifest_path = args.get(2).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: legal_benchmark_sweep <sweep_config.json> <manifest.json>",
        )
    })?;
    let config = serde_json::from_slice::<LegalBenchmarkSweepConfig>(&fs::read(PathBuf::from(
        config_path,
    ))?)?;
    let mut executor = MockLegalBenchmarkSweepExecutor::default();
    let manifest = run_legal_benchmark_sweep(&config, None, &mut executor)?;
    fs::write(
        PathBuf::from(manifest_path),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(())
}
