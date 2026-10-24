//! The zone launcher: Rust Native cards, one per zone.
use super::*;
use rust_native::style::{Space, Style};
use rust_native::{Activation, Axis, Element, Node, TextRole, ValidatedView, View};
use serde::Serialize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

/// Where `refresh` reads the public catalog by default.
pub const ZONE_CATALOG_URL: &str = "https://openagents.com/v1/zones";
const MAX_FETCH_BYTES: usize = aethyrforge_host::MAX_ZONE_CATALOG_BYTES;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) enum LaunchIntent {
    Open(String),
}

/// What the shell does after a card is chosen.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ZoneLaunch {
    /// Show the app's existing coding-agent surfaces (sessions, computers,
    /// transcripts). Nothing is downloaded.
    Grid,
    /// Open this zone in the web runner (`CoreZones::runner`).
    Web { zone_id: String },
}

pub(crate) struct Launcher {
    pub zones: Vec<Zone>,
    instance: String,
    revision: u64,
    view: Option<ValidatedView<LaunchIntent>>,
}
impl Launcher {
    pub fn new(catalog: &ZoneCatalog) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self {
            zones: zones(catalog),
            instance: format!("zones-launcher-{}", NEXT.fetch_add(1, Ordering::Relaxed)),
            revision: 0,
            view: None,
        }
    }
    pub fn frame(&mut self) -> Result<String, ZoneError> {
        self.revision += 1;
        let mut children = vec![
            text("title", "Zones", TextRole::Heading),
            text(
                "subtitle",
                "Places you can open from here.",
                TextRole::Status,
            ),
        ];
        children.extend(self.zones.iter().map(|z| card(&z.manifest)));
        let view = View::new(
            &self.instance,
            self.revision,
            Node {
                key: "zones".into(),
                style: Style {
                    gap: Some(Space::Lg),
                    ..Style::default()
                },
                element: Element::Stack {
                    axis: Axis::Vertical,
                    children,
                },
            },
        )
        .validate()
        .map_err(|_| invalid("The zone list could not be drawn."))?;
        let bytes = view
            .to_json()
            .map_err(|_| invalid("The zone list could not be drawn."))?;
        self.view = Some(view);
        String::from_utf8(bytes).map_err(|_| invalid("The zone list could not be drawn."))
    }
    pub fn activate(&self, activation: &Activation) -> Result<ZoneLaunch, ZoneError> {
        let view = self
            .view
            .as_ref()
            .ok_or_else(|| invalid("The zone list changed. Try again."))?;
        let LaunchIntent::Open(id) = view
            .activate(activation)
            .map_err(|_| invalid("The zone list changed. Try again."))?;
        let zone = self
            .zones
            .iter()
            .find(|z| &z.manifest.id == id)
            .ok_or_else(|| invalid("That zone is no longer listed."))?;
        Ok(match zone.manifest.runner {
            ZoneRunnerKind::Builtin => ZoneLaunch::Grid,
            ZoneRunnerKind::Web => ZoneLaunch::Web {
                zone_id: zone.manifest.id.clone(),
            },
        })
    }
}

fn text(key: &str, value: &str, role: TextRole) -> Node<LaunchIntent> {
    Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Text {
            value: value.into(),
            role,
        },
    }
}

/// One zone card: title, what it is, how it opens, and an Open button.
fn card(z: &ZoneManifest) -> Node<LaunchIntent> {
    let how = match z.runner {
        ZoneRunnerKind::Builtin => "Built in. Works offline.",
        ZoneRunnerKind::Web => "Opens in the app. Needs a connection.",
    };
    Node {
        key: format!("zone:{}", z.id),
        style: Style {
            gap: Some(Space::Sm),
            padding_top: Some(Space::Md),
            padding_bottom: Some(Space::Md),
            padding_start: Some(Space::Md),
            padding_end: Some(Space::Md),
            radius: Some(16),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Vertical,
            children: vec![
                text(&format!("title:{}", z.id), &z.title, TextRole::Heading),
                text(&format!("summary:{}", z.id), &z.summary, TextRole::Body),
                text(&format!("how:{}", z.id), how, TextRole::Status),
                Node {
                    key: format!("open:{}", z.id),
                    style: Style::default(),
                    element: Element::Button {
                        label: format!("Open {}", z.title),
                        enabled: true,
                        icon: None,
                        shortcut: None,
                        intent: LaunchIntent::Open(z.id.clone()),
                    },
                },
            ],
        },
    }
}

/// The launcher the iOS and Android shells present.
#[derive(uniffi::Object)]
pub struct CoreZones {
    state: Mutex<Launcher>,
}
impl CoreZones {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Launcher>, ZoneError> {
        self.state
            .lock()
            .map_err(|_| invalid("The zone list is unavailable."))
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl CoreZones {
    /// The launcher over the bundled pins; works offline.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(Launcher::new(&bundled_catalog())),
        })
    }
    pub fn zones(&self) -> Vec<ZoneManifest> {
        self.lock()
            .map(|s| s.zones.iter().map(|z| z.manifest.clone()).collect())
            .unwrap_or_default()
    }
    /// The launcher's Rust Native view (JSON). Each call is a new revision.
    pub fn launcher(&self) -> Result<String, ZoneError> {
        self.lock()?.frame()
    }
    /// Resolve a tapped card against the view it was drawn from.
    pub fn activate(
        &self,
        instance: String,
        revision: u64,
        node: String,
    ) -> Result<ZoneLaunch, ZoneError> {
        self.lock()?.activate(&Activation {
            instance,
            revision,
            node,
        })
    }
    /// Replace the web zones with a fetched catalog body. The Grid stays.
    pub fn apply_catalog(&self, json: String) -> Result<(), ZoneError> {
        let catalog = parse_catalog(&json)?;
        let mut s = self.lock()?;
        s.zones = zones(&catalog);
        Ok(())
    }
    /// Fetch the public catalog and apply it. On any failure the current
    /// list stays, so the launcher never empties.
    pub async fn refresh(&self) -> Result<(), ZoneError> {
        let unavailable = || ZoneError::Unavailable {
            message: "The zone list could not be updated.".into(),
        };
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| unavailable())?;
        let response = client
            .get(ZONE_CATALOG_URL)
            .send()
            .await
            .map_err(|_| unavailable())?;
        if !response.status().is_success()
            || response
                .content_length()
                .is_some_and(|n| n as usize > MAX_FETCH_BYTES)
        {
            return Err(unavailable());
        }
        let bytes = response.bytes().await.map_err(|_| unavailable())?;
        if bytes.len() > MAX_FETCH_BYTES {
            return Err(unavailable());
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| unavailable())?;
        self.apply_catalog(text.into())
    }
    /// A web runner for one listed web zone.
    pub fn runner(&self, zone_id: String) -> Result<Arc<CoreZoneRunner>, ZoneError> {
        let s = self.lock()?;
        let zone = s
            .zones
            .iter()
            .find(|z| z.manifest.id == zone_id)
            .ok_or_else(|| invalid("That zone is no longer listed."))?;
        CoreZoneRunner::open(zone.clone()).map(Arc::new)
    }
}
