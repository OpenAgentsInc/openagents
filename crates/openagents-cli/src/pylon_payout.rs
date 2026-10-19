//! This computer's OpenAgents wallet (Spark) for `openagents pylon`: the
//! address `payout set` uses when given none and `start` offers, and the
//! balance `earnings` shows when earnings go to it. The same wallet as
//! `openagents wallet` and the phone. `crates/pylon` holds no wallet; this
//! reaches it through `pylon::earnings::SparkWallet`.

use openagents_spark::computer;
use openagents_spark::model::Node as _;

/// The Spark wallet under `openagents_spark::computer::home()`.
pub(crate) struct Spark;

/// Run `work` on a plain thread: the wallet runs its own blocking runtime,
/// and `openagents pylon` calls this from inside its Tokio runtime.
fn detached<T: Send>(work: impl FnOnce() -> Result<T, String> + Send) -> Result<T, String> {
    std::thread::scope(|scope| {
        scope
            .spawn(work)
            .join()
            .unwrap_or_else(|_| Err("the wallet thread panicked".into()))
    })
}

impl pylon::earnings::SparkWallet for Spark {
    fn address(&self) -> Result<Option<String>, String> {
        let home = computer::home();
        if !computer::has_seed(&home) {
            return Ok(None);
        }
        detached(|| computer::open(&home)?.spark_address().map(Some))
    }

    fn balance_sats(&self) -> Result<u64, String> {
        detached(|| {
            let node = computer::open(&computer::home())?;
            node.sync()?;
            node.balance()
        })
    }
}
