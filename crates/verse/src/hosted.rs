//! Shared native presentation of admitted social snapshots; owns no gameplay or Studio authority.
use crate::{
    controller::PlayerController,
    mesh::{Mesh, Vertex},
    runtime::WorldRuntime,
};
use coder_ui::theme::Intensity;
use glam::{Mat4, Quat, Vec3};
use verse_world::{
    play::social::{Profile, State, Zone},
    service::{replica::Buffer, wire::Life},
};

pub(crate) struct Projection {
    pub instance: u64,
    pub digest: [u8; 32],
    pub tick: Option<u64>,
    pub state: Option<verse_world::service::wire::State>,
    pub owned: Option<Life>,
}
impl WorldRuntime {
    /// Selects a host-admitted destination before accepting its first snapshot.
    /// The caller supplies the profile from its verified content manifest.
    pub fn enter_hosted_social(&mut self, instance: u64, profile: &Profile) -> Result<(), String> {
        let digest = profile.digest()?;
        let mesh = geometry_mesh(profile)?;
        let revision = self
            .zone_revision
            .checked_add(1)
            .ok_or("Zone revision exhausted")?;
        self.zone_cancel_loading();
        self.stop_local_studio();
        self.zone_state = crate::zones::State::default();
        self.cancel_navigation();
        self.ball = None;
        self.zone = match profile.zone {
            Zone::Plaza => crate::zones::ZoneId::Plaza,
            Zone::Everglade => crate::zones::ZoneId::Everglade,
        };
        self.zone_revision = revision;
        self.world = crate::world::World {
            mesh,
            blockers: vec![],
        };
        self.hosted = Some(Projection {
            instance,
            digest,
            tick: None,
            state: None,
            owned: None,
        });
        Ok(())
    }
    /// Accepts only validated replica state for the explicitly admitted destination.
    pub fn apply_hosted_social(&mut self, replica: &Buffer) -> Result<(), String> {
        let state = replica
            .latest()
            .ok_or("Hosted social snapshot is missing")?;
        let social = state
            .social
            .as_ref()
            .ok_or("Authority did not serve a social profile")?;
        let tick = replica.tick().ok_or("Hosted social tick is missing")?;
        let projection = self
            .hosted
            .as_mut()
            .ok_or("No hosted destination was admitted")?;
        state.validate_control(projection.instance, &replica.control().cloned())?;
        if social.profile.digest()? != projection.digest
            || projection.tick.is_some_and(|old| tick < old)
        {
            return Err("Hosted social content or clock mismatch".into());
        }
        projection.tick = Some(tick);
        projection.state = Some(state.clone());
        projection.owned = replica.control().map(|c| c.life);
        self.restore_hosted_player();
        Ok(())
    }
    pub fn hosted_social_state(&self) -> Option<&State> {
        self.hosted.as_ref()?.state.as_ref()?.social.as_ref()
    }
    /// Leaving a host drops its projection before restoring a local world.
    pub fn leave_hosted_social(&mut self) {
        if self.hosted.is_some() {
            let revision = self.zone_revision.saturating_add(1);
            let local = if self.is_unoccupied() {
                Self::unoccupied()
            } else if self.is_bare() {
                Self::bare()
            } else {
                Self::new()
            };
            *self = local;
            self.zone_revision = revision;
        }
    }
    pub(crate) fn restore_hosted_player(&mut self) {
        let Some(p) = &self.hosted else {
            return;
        };
        let Some(actor) = p.state.as_ref().and_then(|s| {
            s.presentation
                .actors
                .iter()
                .find(|a| Some(a.life) == p.owned)
        }) else {
            return;
        };
        self.player = PlayerController::new(actor.actor.position, actor.actor.yaw);
    }
    pub(crate) fn hosted_mesh(&self) -> Mesh {
        let mut mesh = Mesh::default();
        let Some(p) = &self.hosted else {
            return mesh;
        };
        let Some(state) = &p.state else {
            return mesh;
        };
        for actor in &state.presentation.actors {
            if actor.visible && actor.health > 0 {
                mesh.extend(&crate::avatar::figure(
                    actor.actor.position,
                    Quat::from_rotation_y(actor.actor.yaw),
                    &crate::avatar::Gait::default(),
                    Intensity::Full,
                ));
            }
        }
        if let Some(social) = &state.social {
            for a in &social.studio {
                mesh.extend(&crate::avatar::figure(
                    Vec3::from_array(a.feet),
                    Quat::from_rotation_y(a.yaw),
                    &crate::avatar::Gait::default(),
                    Intensity::Half,
                ));
            }
            for object in &social.profile.objects {
                let occupied = social.occupant(object.id).is_some()
                    || social.studio.iter().any(|a| a.seat == object.id);
                let on = social.switch_on(object.id);
                let at = Vec3::from_array(object.feet) + Vec3::Y * 0.25;
                mesh.cube(
                    Mat4::from_scale_rotation_translation(
                        Vec3::new(0.5, 0.5, 0.5),
                        Quat::from_rotation_y(object.yaw),
                        at,
                    ),
                    if occupied || on {
                        Intensity::Full
                    } else {
                        Intensity::Quarter
                    },
                );
            }
        }
        mesh
    }
}
fn geometry_mesh(profile: &Profile) -> Result<Mesh, String> {
    use physics::queries::GeometrySnapshot;
    profile.validate()?;
    let mut mesh = Mesh::default();
    for shape in &profile.geometry.colliders {
        match &shape.geometry {
            GeometrySnapshot::Box { min, max } => {
                mesh.cube(
                    Mat4::from_scale_rotation_translation(
                        (*max - *min).as_vec3(),
                        Quat::IDENTITY,
                        ((*max + *min) * 0.5).as_vec3(),
                    ),
                    Intensity::Quarter,
                );
            }
            GeometrySnapshot::Triangles { triangles } => {
                for t in triangles {
                    mesh.faces.extend(t.0.map(|p| Vertex {
                        pos: p.as_vec3().to_array(),
                        color: crate::palette::field(),
                        fog: 1.,
                    }));
                }
            }
            _ => return Err("Unsupported hosted social presentation geometry".into()),
        }
    }
    Ok(mesh)
}
/// Connects the existing authenticated client and replica to the shared native runtime.
#[cfg(feature = "remote-chamber")]
pub struct Client {
    client: verse_world::service::client::Client,
    replica: Buffer,
    revision: u64,
}
#[cfg(feature = "remote-chamber")]
impl Client {
    pub async fn attach(
        mut client: verse_world::service::client::Client,
        world: &mut WorldRuntime,
        instance: u64,
        profile: &Profile,
    ) -> Result<Self, String> {
        let response = client.replicated_snapshot().await?;
        let mut replica = Buffer::new(instance, 30.)?;
        replica.push(&response)?;
        // Verify the response before replacing an existing world.
        let state = replica.latest().ok_or("Missing hosted snapshot")?;
        if state
            .social
            .as_ref()
            .ok_or("Unsupported hosted rules")?
            .profile
            .digest()?
            != profile.digest()?
        {
            return Err("Hosted social content mismatch".into());
        }
        world.enter_hosted_social(instance, profile)?;
        world.apply_hosted_social(&replica)?;
        Ok(Self {
            client,
            replica,
            revision: world.zone_revision,
        })
    }
    /// As [`Self::attach`], for a viewer whose NIP-HOST grant holds
    /// `rights`: the world records them as the studio grant, so the `world`
    /// right alone opens no studio panel ([`WorldRuntime::set_studio_grant`]).
    /// The instance's studio seats come from the authority's snapshots,
    /// which every viewer draws alike.
    pub async fn attach_granted(
        client: verse_world::service::client::Client,
        world: &mut WorldRuntime,
        instance: u64,
        profile: &Profile,
        rights: Vec<coder_access::Right>,
    ) -> Result<Self, String> {
        let attached = Self::attach(client, world, instance, profile).await?;
        world.set_studio_grant(Some(rights));
        Ok(attached)
    }
    fn check_attachment(&self, world: &WorldRuntime) -> Result<(), String> {
        if world.zone_revision != self.revision
            || world
                .hosted
                .as_ref()
                .is_none_or(|p| p.instance != self.client.instance())
        {
            return Err("Hosted client attachment has been retired".into());
        }
        Ok(())
    }
    /// Closes the admitted transport before restoring the caller's local world mode.
    pub async fn leave(mut self, world: &mut WorldRuntime) -> Result<(), String> {
        self.check_attachment(world)?;
        let result = self.client.close().await;
        world.leave_hosted_social();
        result
    }
    pub async fn refresh(&mut self, world: &mut WorldRuntime) -> Result<(), String> {
        self.check_attachment(world)?;
        let response = self.client.replicated_snapshot().await?;
        self.replica.push(&response)?;
        world.apply_hosted_social(&self.replica)
    }
    pub async fn movement(
        &mut self,
        world: &mut WorldRuntime,
        axes: [f32; 2],
        yaw: f32,
    ) -> Result<verse_world::service::wire::Response, String> {
        self.check_attachment(world)?;
        let response = self
            .client
            .command(verse_world::Intent::Move { axes, yaw })
            .await?;
        self.refresh(world).await?;
        Ok(response)
    }
    pub async fn interact(
        &mut self,
        world: &mut WorldRuntime,
        action: verse_world::play::social::Action,
    ) -> Result<verse_world::service::wire::Response, String> {
        self.check_attachment(world)?;
        let response = self.client.social(action).await?;
        self.refresh(world).await?;
        Ok(response)
    }
}

/// A hosted instance joined from a frame loop that never waits on the
/// network: a worker thread owns the attached connection, sends the
/// latest steering as movement commands, and reads replicated snapshots;
/// the frame applies whatever arrived ([`Link::pump`]). Dropping the link
/// closes the connection.
#[cfg(feature = "remote-chamber")]
pub struct Link {
    steer: std::sync::Arc<std::sync::Mutex<[f32; 3]>>,
    updates: std::sync::mpsc::Receiver<Result<verse_world::service::wire::Response, String>>,
    replica: Buffer,
    instance: u64,
    revision: u64,
}

#[cfg(feature = "remote-chamber")]
impl Link {
    /// How often the worker sends movement and asks for a snapshot.
    pub const PERIOD: std::time::Duration = std::time::Duration::from_millis(33);

    /// Hands an attached `client` to a worker thread that drives it on
    /// `runtime`, the runtime that opened its connection. `world` must
    /// still be attached to it.
    ///
    /// # Errors
    /// Returns a message when the attachment is retired or the worker
    /// cannot start.
    pub fn start(
        attached: Client,
        world: &WorldRuntime,
        runtime: tokio::runtime::Runtime,
    ) -> Result<Self, String> {
        use std::sync::{Arc, Mutex, mpsc};
        attached.check_attachment(world)?;
        let Client {
            mut client,
            replica,
            revision,
        } = attached;
        let instance = client.instance();
        let steer = Arc::new(Mutex::new([0.0f32; 3]));
        let (send, updates) = mpsc::channel();
        let steering = steer.clone();
        std::thread::Builder::new()
            .name("verse-hosted".into())
            .spawn(move || {
                runtime.block_on(async move {
                    let mut moving = false;
                    loop {
                        let [x, z, yaw] = *steering.lock().unwrap_or_else(|e| e.into_inner());
                        let axes = [x, z];
                        // A held direction moves every period; letting go
                        // sends one stop.
                        if axes != [0.0; 2] || moving {
                            moving = axes != [0.0; 2];
                            if let Err(error) = client
                                .command(verse_world::Intent::Move { axes, yaw })
                                .await
                            {
                                let _ = send.send(Err(error));
                                return;
                            }
                        }
                        let update = client.replicated_snapshot().await;
                        let failed = update.is_err();
                        if send.send(update).is_err() || failed {
                            let _ = client.close().await;
                            return;
                        }
                        tokio::time::sleep(Self::PERIOD).await;
                    }
                });
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            steer,
            updates,
            replica,
            instance,
            revision,
        })
    }

    /// How fast `A` and `D` turn a hosted avatar, rad/s.
    pub const TURN_RATE: f32 = 2.5;

    /// Steers from the keyboard: `W` and `S` walk along `heading`, `Q` and
    /// `E` strafe, and `A` and `D` turn `heading` (the controller's yaw,
    /// whose forward is `(sin, cos)`) over `dt` seconds.
    pub fn steer_input(&self, input: &crate::controller::InputState, heading: &mut f32, dt: f32) {
        let axis = |plus: bool, minus: bool| f32::from(u8::from(plus)) - f32::from(u8::from(minus));
        let turn = axis(input.left, input.right);
        if dt.is_finite() {
            *heading += turn * Self::TURN_RATE * dt.clamp(0.0, 0.1);
        }
        // The chamber's forward is `(-sin, -cos)` of its yaw: half a turn
        // from the controller's.
        self.steer(
            [
                axis(input.strafe_right, input.strafe_left),
                axis(input.forward, input.backward),
            ],
            *heading + std::f32::consts::PI,
        );
    }

    /// Steers the owned avatar along `axes` (strafe and forward, each
    /// -1..1) facing the chamber's `yaw`, from the next period on.
    pub fn steer(&self, axes: [f32; 2], yaw: f32) {
        let axes = axes.map(|v| {
            if v.is_finite() {
                v.clamp(-1.0, 1.0)
            } else {
                0.0
            }
        });
        let yaw = if yaw.is_finite() { yaw } else { 0.0 };
        *self.steer.lock().unwrap_or_else(|e| e.into_inner()) = [axes[0], axes[1], yaw];
    }

    /// Applies the snapshots that arrived since the last call to `world`.
    /// Returns whether one did.
    ///
    /// # Errors
    /// Returns the connection's failure, or a message when `world` left
    /// the instance or a snapshot is refused.
    pub fn pump(&mut self, world: &mut WorldRuntime) -> Result<bool, String> {
        if world.zone_revision != self.revision
            || world
                .hosted
                .as_ref()
                .is_none_or(|p| p.instance != self.instance)
        {
            return Err("Hosted link attachment has been retired".into());
        }
        let mut applied = false;
        for update in self.updates.try_iter() {
            self.replica.push(&update?)?;
            applied = true;
        }
        if applied {
            world.apply_hosted_social(&self.replica)?;
        }
        Ok(applied)
    }
}

/// A hosted Everglade instance to join over a NIP-REACH channel, as
/// desktop Verse's `--join FILE` names it: where the chamber listens,
/// its instance, the Coder host's key, and the computers store with this
/// device's key and the grant that host signed for it (it must hold
/// `world`).
#[cfg(feature = "remote-chamber")]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Join {
    pub address: std::net::SocketAddr,
    pub instance: u64,
    /// The Coder host's x-only public key.
    pub host: String,
    /// The computers store with `device.key` and `computers.json`.
    pub store: std::path::PathBuf,
    /// The content identity the chamber reports, as hex.
    #[serde(default)]
    pub content: Option<String>,
    /// Open the channel over a WebSocket upgrade.
    #[serde(default)]
    pub websocket: bool,
}

#[cfg(feature = "remote-chamber")]
impl Join {
    /// Reads a join file of at most 64 KiB.
    ///
    /// # Errors
    /// Returns a message when the file is unreadable or malformed.
    pub fn read(path: &std::path::Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if bytes.len() > 64 * 1024 {
            return Err(format!("{}: exceeds 64 KiB", path.display()));
        }
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// This device's key and the grant the host signed for it, with the
    /// grant's rights.
    fn grant(
        &self,
    ) -> Result<
        (
            verse_world::service::reach::ClientConfig,
            Vec<coder_access::Right>,
        ),
        String,
    > {
        let key = std::fs::read_to_string(self.store.join("device.key"))
            .map_err(|_| "The computers store has no device key")?;
        let device: secp256k1::SecretKey = key.trim().parse().map_err(|_| "Invalid device key")?;
        let saved: serde_json::Value = serde_json::from_slice(
            &std::fs::read(self.store.join("computers.json"))
                .map_err(|_| "The computers store has no saved computers")?,
        )
        .map_err(|_| "The saved computers record is unreadable")?;
        let grant = saved["hosts"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|host| &host["access"]["grant"])
            .find(|grant| grant["host"].as_str() == Some(self.host.as_str()))
            .ok_or("This device holds no grant from that host")?;
        let device_key = secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), &device)
            .x_only_public_key()
            .0
            .to_string();
        if grant["device"].as_str() != Some(device_key.as_str()) {
            return Err("The saved grant names another device key".into());
        }
        let rights: Vec<coder_access::Right> = serde_json::from_value(grant["rights"].clone())
            .map_err(|_| "The saved grant's rights are unreadable")?;
        Ok((
            verse_world::service::reach::ClientConfig {
                device,
                host: self.host.clone(),
                grant: grant["grant"]
                    .as_str()
                    .ok_or("The saved grant has no ID")?
                    .to_owned(),
                epoch: grant["epoch"]
                    .as_u64()
                    .ok_or("The saved grant has no epoch")?,
                // A chamber's channel names its instance as the generation.
                generation: self.instance,
                timeout: std::time::Duration::from_secs(10),
            },
            rights,
        ))
    }

    /// Joins the instance as this device, enters its Everglade profile in
    /// `world` with the grant's rights as the studio grant, and hands the
    /// connection to a [`Link`].
    ///
    /// # Errors
    /// Returns a message when the grant, the channel, or the instance's
    /// content refuses.
    pub fn open(&self, world: &mut WorldRuntime) -> Result<Link, String> {
        use verse_world::service::reach;
        let (config, rights) = self.grant()?;
        let content = self
            .content
            .as_deref()
            .map(|hex| {
                let bytes: Vec<u8> = (0..hex.len())
                    .step_by(2)
                    .map(|i| {
                        hex.get(i..i + 2)
                            .and_then(|b| u8::from_str_radix(b, 16).ok())
                    })
                    .collect::<Option<_>>()
                    .ok_or("The content identity is not hex")?;
                <[u8; 32]>::try_from(bytes).map_err(|_| "The content identity is not 32 bytes")
            })
            .transpose()?;
        let profile = verse_world::social::hosted::everglade_profile()?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let attached = runtime.block_on(async {
            let socket = tokio::net::TcpStream::connect(self.address)
                .await
                .map_err(|e| format!("Cannot reach {}: {e}", self.address))?;
            let _ = socket.set_nodelay(true);
            let client = if self.websocket {
                let socket = reach::websocket::client(&format!("ws://{}/", self.address), socket)
                    .await
                    .map_err(|e| format!("Chamber channel refused: {e}"))?;
                reach::join(socket, &config, self.instance, content).await?
            } else {
                reach::join(socket, &config, self.instance, content).await?
            };
            Client::attach_granted(client, world, self.instance, &profile, rights).await
        })?;
        Link::start(attached, world, runtime)
    }
}

#[cfg(all(test, feature = "remote-chamber"))]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use verse_world::{
        play::{
            Game,
            social::{Action, Kind, Object},
        },
        service::{Chamber, auth::Gateway, wire::Reply},
    };
    fn profile(zone: Zone) -> Profile {
        use physics::queries::{ColliderKey, Life, Mesh, MeshCollider, Scene, Usage};
        let mut geometry = Scene::default();
        geometry
            .insert(MeshCollider {
                key: ColliderKey {
                    life: Life {
                        instance: 0,
                        entity: 0,
                        generation: 0,
                    },
                    shape: 0,
                },
                layers: 1,
                usage: Usage::Blocking,
                mesh: Mesh::from_box(
                    glam::DVec3::new(-12., -1., -12.),
                    glam::DVec3::new(12., 0., 12.),
                )
                .unwrap(),
            })
            .unwrap();
        Profile {
            revision: 1,
            zone,
            geometry: geometry.snapshot(0).unwrap(),
            objects: vec![
                Object {
                    id: 1,
                    feet: [0., 0., 0.],
                    yaw: 0.,
                    kind: Kind::Seat,
                },
                Object {
                    id: 2,
                    feet: [1., 0., 0.],
                    yaw: 0.,
                    kind: Kind::Switch,
                },
            ],
        }
    }
    fn key(n: u8) -> secp256k1::Keypair {
        secp256k1::Keypair::from_secret_key(
            &secp256k1::Secp256k1::new(),
            &secp256k1::SecretKey::from_byte_array([n; 32]).unwrap(),
        )
    }
    fn gateway(instance: u64, zone: Zone) -> Arc<Mutex<Gateway>> {
        let mut scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        scene.actors.retain(|a| a.model == "adventurer");
        scene.actors[0].position = Vec3::ZERO;
        scene.cut_at = 0.;
        scene.cues.clear();
        scene.collision_profile = None;
        let game = Game::social_in(scene, instance, profile(zone)).unwrap();
        let mut gateway = Gateway::new(Chamber::new(game).unwrap()).unwrap();
        gateway
            .enroll_primary(key(31).x_only_public_key().0.serialize())
            .unwrap();
        gateway
            .enroll_spectator(key(32).x_only_public_key().0.serialize())
            .unwrap();
        Arc::new(Mutex::new(gateway))
    }
    /// In-memory transport runs real framed authentication and SDK IO; it models an already verified host.
    async fn connect(
        gateway: Arc<Mutex<Gateway>>,
        instance: u64,
        n: u8,
    ) -> verse_world::service::client::Client {
        use verse_world::service::net::{read_frame, write_frame};
        let (client, mut server) = tokio::io::duplex(2 * 1024 * 1024);
        let (id, hello) = gateway.lock().unwrap().open_json(0).unwrap();
        tokio::spawn(async move {
            write_frame(&mut server, &hello, 2 * 1024 * 1024)
                .await
                .unwrap();
            while let Ok(bytes) = read_frame(&mut server, 16 * 1024).await {
                let response = gateway
                    .lock()
                    .unwrap()
                    .dispatch_json(id, 0, &bytes)
                    .unwrap();
                if write_frame(&mut server, &response, 2 * 1024 * 1024)
                    .await
                    .is_err()
                {
                    break;
                }
            }
            let _ = gateway.lock().unwrap().close(id);
        });
        verse_world::service::client::Client::connect_stream(
            Box::new(client),
            instance,
            None,
            &key(n),
        )
        .await
        .unwrap()
    }
    #[tokio::test]
    async fn hosted_social_two_native_viewers_share_authority_and_reject_old_zone_updates() {
        let gateway = gateway(3001, Zone::Plaza);
        gateway.lock().unwrap().tick(1. / 30.).unwrap();
        let one = connect(gateway.clone(), 3001, 31).await;
        let two = connect(gateway.clone(), 3001, 32).await;
        let mut player = WorldRuntime::bare();
        let mut observer = WorldRuntime::unoccupied();
        let mut one = Client::attach(one, &mut player, 3001, &profile(Zone::Plaza))
            .await
            .unwrap();
        let mut two = Client::attach(two, &mut observer, 3001, &profile(Zone::Plaza))
            .await
            .unwrap();
        assert!(matches!(
            one.interact(&mut player, Action::Sit { object: 1 })
                .await
                .unwrap()
                .body,
            Reply::Accepted
        ));
        two.refresh(&mut observer).await.unwrap();
        assert_eq!(player.hosted_social_state(), observer.hosted_social_state());
        assert_eq!(player.dynamic_mesh().faces, observer.dynamic_mesh().faces);
        let at = player.player.pos;
        for _ in 0..20 {
            player.tick(
                &crate::controller::InputState {
                    forward: true,
                    jump: true,
                    ..Default::default()
                },
                0.05,
            );
        }
        assert_eq!(at, player.player.pos);
        assert!(player.set_spawn(Vec3::new(10., 0., 0.), 0.).is_err());
        assert!(player.zone_intent(crate::zones::Intent::Return).is_err());
        assert!(
            player
                .studio_send(coder_access::Operation::StudioSnapshot {})
                .is_err()
        );
        assert!(matches!(
            one.movement(&mut player, [1., 0.], 0.).await.unwrap().body,
            Reply::Accepted
        ));
        gateway.lock().unwrap().tick(0.05).unwrap();
        one.refresh(&mut player).await.unwrap();
        two.refresh(&mut observer).await.unwrap();
        assert_ne!(at, player.player.pos);
        assert_eq!(player.dynamic_mesh().faces, observer.dynamic_mesh().faces);
        assert!(player.hosted_social_state().unwrap().occupants.is_empty());
        let old = one.replica.latest().unwrap().clone();
        let replacement = connect(gateway.clone(), 3001, 32).await;
        let mut replacement = Client::attach(replacement, &mut player, 3001, &profile(Zone::Plaza))
            .await
            .unwrap();
        assert!(
            one.interact(&mut player, Action::Toggle { object: 2 })
                .await
                .is_err()
        );
        assert!(
            !gateway
                .lock()
                .unwrap()
                .game()
                .social_state()
                .unwrap()
                .switch_on(2)
        );
        assert!(one.refresh(&mut player).await.is_err());
        replacement.refresh(&mut player).await.unwrap();
        player
            .enter_hosted_social(3002, &profile(Zone::Everglade))
            .unwrap();
        assert!(one.refresh(&mut player).await.is_err());
        assert!(player.hosted_social_state().is_none());
        let destination = gateway_for_destination();
        let new = connect(destination, 3002, 31).await;
        let mut new = Client::attach(new, &mut player, 3002, &profile(Zone::Everglade))
            .await
            .unwrap();
        assert_eq!(player.zone, crate::zones::ZoneId::Everglade);
        let before = player.dynamic_mesh().faces;
        assert!(one.refresh(&mut player).await.is_err());
        assert_eq!(before, player.dynamic_mesh().faces);
        assert_ne!(
            old.social.unwrap().profile.zone,
            player.hosted_social_state().unwrap().profile.zone
        );
        assert!(matches!(
            new.interact(&mut player, Action::Toggle { object: 2 })
                .await
                .unwrap()
                .body,
            Reply::Accepted
        ));
        new.leave(&mut player).await.unwrap();
        assert!(player.is_bare());
        observer.leave_hosted_social();
        assert!(observer.is_unoccupied());
        assert!(!player.is_hosted());
        assert_eq!(player.zone, crate::zones::ZoneId::Plaza);
        println!(
            "native social acceptance: two real framed SDK viewers agree, local movement/placement/transition/Studio operations refused, old source projection fenced, explicit destination/exit"
        );
    }
    fn gateway_for_destination() -> Arc<Mutex<Gateway>> {
        gateway(3002, Zone::Everglade)
    }
    #[tokio::test]
    async fn a_linked_everglade_viewer_walks_sees_the_studio_and_opens_no_panel_with_world_alone() {
        let gateway = gateway(3003, Zone::Everglade);
        gateway.lock().unwrap().tick(1. / 30.).unwrap();
        gateway
            .lock()
            .unwrap()
            .publish_social_studio(vec![verse_world::play::social::SeatActor {
                seat: 1,
                feet: [0.5, 0., 0.5],
                yaw: 0.,
            }])
            .unwrap();
        let one = connect(gateway.clone(), 3003, 31).await;
        let mut world = WorldRuntime::bare();
        let attached = Client::attach_granted(
            one,
            &mut world,
            3003,
            &profile(Zone::Everglade),
            vec![coder_access::Right::World],
        )
        .await
        .unwrap();
        // The `world` right walks; it opens no studio panel.
        assert!(!world.studio().access().read);
        assert_eq!(world.hosted_social_state().unwrap().studio.len(), 1);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let mut link = Link::start(attached, &world, runtime).unwrap();
        let at = world.player.pos;
        link.steer([0., 1.], 0.);
        let started = std::time::Instant::now();
        while world.player.pos == at {
            assert!(started.elapsed() < std::time::Duration::from_secs(20));
            gateway.lock().unwrap().tick(1. / 30.).unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            link.pump(&mut world).unwrap();
        }
        // Leaving the instance retires the link.
        world.leave_hosted_social();
        assert!(link.pump(&mut world).is_err());
    }
}
