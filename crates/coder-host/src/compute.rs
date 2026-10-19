//! Shared compute: the host is the pylon. `openagents pylon up|down` and
//! `openagents host share on|off|status` record the owner's choice
//! (`pylon::share`), and `serve` runs the supervisor that keeps the pylon
//! running while it is on: the Psionic engine children `up` asked for
//! (restarted with backoff when one crashes), the self-check, and the
//! provider (`pylon::online::supervise`). Because the host service starts
//! at login, the pylon comes back after a reboot. Pool jobs take a `pylon`
//! lease at `background` priority, so the owner's work comes first. Only
//! this user's own host shares; a host under another root, as a test runs
//! it, never does.

use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::oneshot;

/// How often the supervisor looks at the setting.
const EVERY: Duration = Duration::from_secs(5);

type Running = (oneshot::Sender<()>, tokio::task::JoinHandle<()>);

static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

/// Run `openagents host share WORDS`.
#[must_use]
pub fn command(words: &[String]) -> u8 {
    pylon::share::command(words, &pylon::home(), &pylon::cli::host_slug())
}

/// Start the shared-compute supervisor once for this process.
pub(crate) fn start() {
    let Ok(mut running) = RUNNING.lock() else {
        return;
    };
    if running.is_some() {
        return;
    }
    let machine = match pylon::share::host_machine() {
        Ok(machine) => machine,
        Err(error) => {
            eprintln!("openagents host: shared compute is off: {error}");
            return;
        }
    };
    let (halt, halted) = oneshot::channel::<()>();
    let task = tokio::spawn(pylon::share::supervise(
        pylon::home(),
        machine,
        EVERY,
        |line| eprintln!("openagents host: {line}"),
        async {
            let _ = halted.await;
        },
    ));
    *running = Some((halt, task));
}

/// How long the host waits, when it stops, for the pylon's jobs to finish,
/// its offline beacon, and its engines to stop. The engines are killed
/// with the host after that.
const STOP_WAIT: Duration = Duration::from_secs(30);

/// Stop the supervisor, which lets admitted jobs finish, publishes an
/// offline beacon when the pylon was serving, and stops its engines.
pub(crate) async fn stop() {
    let held = RUNNING.lock().ok().and_then(|mut running| running.take());
    if let Some((halt, task)) = held {
        let _ = halt.send(());
        let _ = tokio::time::timeout(STOP_WAIT, task).await;
    }
}
