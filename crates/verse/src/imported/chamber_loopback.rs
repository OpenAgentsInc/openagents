//! An in-process chamber host for tests: the RITUAL scene's authority on its
//! own thread, reached over in-memory streams instead of TLS. It exercises
//! the same client, worker, and session a real host does, and it can sever
//! every connection to stand in for a host that goes away.
use secp256k1::{Keypair, Secp256k1, SecretKey};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    io::DuplexStream,
    sync::{mpsc, watch},
};
use verse_engine::director::Scene;
use verse_world::service::{
    Chamber,
    auth::{ConnectionId, Gateway},
    client::Client,
    net::{read_frame, write_frame},
    wire::{MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES},
};

/// The instance every loopback chamber runs.
pub const INSTANCE: u64 = 160;
const SCENE: &[u8] = include_bytes!("../../../../assets/verse/original/ritual.json");

/// A running loopback host. Dropping it stops the host thread.
pub struct Loopback {
    gateway: Arc<Mutex<Gateway>>,
    streams: mpsc::UnboundedSender<DuplexStream>,
    sever: watch::Sender<u64>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
    key: Keypair,
}

impl Loopback {
    /// Starts the RITUAL chamber with the primary player enrolled under
    /// [`Loopback::key`]. With `dead`, the player starts at zero health.
    ///
    /// # Errors
    /// The scene or the authority cannot be made.
    pub fn start(dead: bool) -> Result<Self, String> {
        let scene = Self::scene()?;
        let mut game = verse_world::play::Game::combat_in(scene.clone(), false, INSTANCE)?;
        game.time = scene.cut_at;
        game.tick(1. / 30., [0.; 2])?;
        if let Some(encounter) = game.encounter.as_mut() {
            // Hostiles hold their casts so a test decides when the player dies.
            encounter.postpone_casts_until(600.)?;
        }
        if dead {
            game.hostile_hit(i32::MAX / 2)?;
            game.tick(1. / 30., [0.; 2])?;
        }
        let mut gateway = Gateway::new(Chamber::new(game)?)?;
        let key = Keypair::from_secret_key(
            &Secp256k1::new(),
            &SecretKey::from_byte_array([101; 32]).map_err(|e| e.to_string())?,
        );
        gateway.enroll_primary(key.x_only_public_key().0.serialize())?;
        let gateway = Arc::new(Mutex::new(gateway));
        let (streams, mut accepted) = mpsc::unbounded_channel::<DuplexStream>();
        let (sever, severed) = watch::channel(0);
        let (stop, mut stopping) = tokio::sync::oneshot::channel();
        let host = gateway.clone();
        let thread = std::thread::Builder::new()
            .name("chamber-loopback".into())
            .spawn(move || {
                let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                runtime.block_on(async move {
                    let started = Instant::now();
                    let mut ticks = tokio::time::interval(Duration::from_millis(33));
                    loop {
                        tokio::select! {
                            _ = &mut stopping => break,
                            stream = accepted.recv() => match stream {
                                Some(stream) => {
                                    tokio::spawn(serve(host.clone(), stream, severed.clone(), started));
                                }
                                None => break,
                            },
                            _ = ticks.tick() => {
                                let mut gateway = host.lock().unwrap_or_else(|p| p.into_inner());
                                if gateway.tick(1. / 30.).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                });
            })
            .map_err(|e| format!("Cannot start the loopback chamber: {e}"))?;
        Ok(Self {
            gateway,
            streams,
            sever,
            stop: Some(stop),
            thread: Some(thread),
            key,
        })
    }

    /// The RITUAL scene the loopback chamber plays.
    ///
    /// # Errors
    /// The bundled scene does not parse.
    pub fn scene() -> Result<Scene, String> {
        Scene::from_json(SCENE)
    }

    /// The primary player's key.
    #[must_use]
    pub fn key(&self) -> &Keypair {
        &self.key
    }

    /// Runs `f` against the authority.
    pub fn with<T>(&self, f: impl FnOnce(&mut Gateway) -> T) -> T {
        f(&mut self.gateway.lock().unwrap_or_else(|p| p.into_inner()))
    }

    /// Connects and authenticates a new client as the primary player, on a
    /// runtime of its own that the caller hands to the session.
    ///
    /// # Errors
    /// The host is gone or refuses the client.
    pub fn connect(&self) -> Result<(Client, tokio::runtime::Runtime), String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let (client, host) = tokio::io::duplex(1 << 20);
        self.streams
            .send(host)
            .map_err(|_| "The loopback chamber stopped".to_string())?;
        let client = runtime.block_on(Client::connect_stream(
            Box::new(client),
            INSTANCE,
            None,
            &self.key,
        ))?;
        Ok((client, runtime))
    }

    /// [`Loopback::connect`] with the content a phone or window mounts: the
    /// generated original pack under `dir`, its atlas, and the scene.
    ///
    /// # Errors
    /// The pack cannot be generated or the client cannot connect.
    pub fn open(&self, dir: &Path) -> Result<crate::ritual::Opened, String> {
        let pack = super::original::generate(dir)?;
        let atlas = super::original::atlas()?;
        let (client, runtime) = self.connect()?;
        Ok(crate::ritual::Opened {
            client,
            runtime,
            pack,
            atlas,
            scene: Self::scene()?,
            dir: dir.to_path_buf(),
        })
    }

    /// Drops every open connection, as a host that goes away would. Later
    /// connections are served again.
    pub fn sever(&self) {
        self.sever.send_modify(|n| *n += 1);
    }
}

impl Drop for Loopback {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

async fn serve(
    gateway: Arc<Mutex<Gateway>>,
    mut stream: DuplexStream,
    mut severed: watch::Receiver<u64>,
    started: Instant,
) {
    severed.mark_unchanged();
    let now = || started.elapsed().as_millis() as u64;
    let Ok((connection, hello)) = gateway
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .open_json(now())
    else {
        return;
    };
    let close = |connection: ConnectionId| {
        let _ = gateway
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .close(connection);
    };
    if write_frame(&mut stream, &hello, MAX_RESPONSE_BYTES)
        .await
        .is_err()
    {
        close(connection);
        return;
    }
    loop {
        let bytes = tokio::select! {
            _ = severed.changed() => break,
            bytes = read_frame(&mut stream, MAX_REQUEST_BYTES) => match bytes {
                Ok(bytes) => bytes,
                Err(_) => break,
            },
        };
        let reply = gateway
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .dispatch_json(connection, now(), &bytes);
        let Ok(reply) = reply else {
            break;
        };
        if write_frame(&mut stream, &reply, MAX_RESPONSE_BYTES)
            .await
            .is_err()
        {
            break;
        }
    }
    close(connection);
}
