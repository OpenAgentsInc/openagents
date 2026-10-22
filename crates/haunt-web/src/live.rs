//! Live mode: the house as a livehost zone that updates while people walk
//! it (`docs/verse/games/the-haunted-house.md`, "Hosting").
//!
//! The tenant's `zone.json` lists the house's global settings
//! (`house.json`), one chunk per area (the area's part of the `haunt.v1`
//! document, placed at the area's middle), and the asset chunks each area
//! draws with: meshes (`chunks/meshes/ID.bin`, a part per shape) and
//! die-cut cards (`chunks/cards/ID.ici`). First play is the global
//! settings, the areas near the arrival and their assets; the other areas
//! stream in by distance. When the channel moves, only objects whose hash
//! is new are fetched, and the page applies them at the top of a frame:
//! new volumes and looks at once, a changed area rebuilt in place.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use haunt_rules::doc::{Area, House};
use icecream_zone::chunk::Part;
use icecream_zone::image::Image;
use icecream_zone::{Kind, Objects, Zone};
use live_web::{Channel, Pointer, fetch_all, fetch_text, now, url_of};
use wasm_bindgen_futures::spawn_local;

/// Areas fetched at once while streaming.
const STREAMING: usize = 4;

/// Asset chunks, decoded.
#[derive(Default)]
pub struct Assets {
    /// Mesh parts named by mesh key (`asset#state`).
    pub meshes: Vec<Part>,
    /// Card images by asset id.
    pub cards: Vec<(String, Image)>,
}

fn decode_assets(files: Vec<(String, Vec<u8>)>) -> Result<Assets, String> {
    let mut out = Assets::default();
    for (path, bytes) in files {
        if path.ends_with(".bin") {
            out.meshes.extend(icecream_zone::chunk::decode(&bytes)?);
        } else if path.ends_with(".ici") {
            let id = path
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .trim_end_matches(".ici")
                .to_owned();
            out.cards.push((id, icecream_zone::image::decode(&bytes)?));
        }
    }
    Ok(out)
}

/// A version fetched and decoded, waiting for the next frame.
pub struct Version {
    pub manifest: String,
    pub seq: u64,
    objects: Objects,
    zone: Zone,
    /// The new global settings, when they changed.
    pub head: Option<House>,
    /// Areas whose chunk changed (or that are new), by id.
    pub areas: Vec<Area>,
    /// Area ids no longer in the house.
    pub removed: Vec<String>,
    pub assets: Assets,
    /// Asset chunk paths this version loaded.
    asset_paths: Vec<String>,
    /// Area chunk paths this version loaded.
    area_paths: Vec<String>,
    /// URLs fetched, to count their bytes, and when the pointer arrived.
    pub urls: Vec<String>,
    pub since: f64,
}

#[derive(Default)]
struct Inbox {
    ready: Vec<Result<Version, String>>,
    streamed: Vec<(String, String, Area, Assets, Vec<String>)>,
    fetching: bool,
    in_flight: usize,
}

/// The zone the page shows.
pub struct Live {
    channel: Rc<Channel>,
    pub tenant: String,
    pub manifest: String,
    pub seq: u64,
    objects: Objects,
    zone: Zone,
    head: House,
    /// Loaded areas by chunk path.
    areas: BTreeMap<String, Area>,
    /// Asset chunks loaded.
    assets: BTreeSet<String>,
    /// Chunks asked for.
    requested: BTreeSet<String>,
    inbox: Rc<RefCell<Inbox>>,
}

async fn fetch_paths(
    objects: &Objects,
    paths: &[String],
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let urls = paths
        .iter()
        .map(|p| url_of(objects, p))
        .collect::<Result<Vec<_>, _>>()?;
    let bodies = fetch_all(&urls).await?;
    Ok(paths.iter().cloned().zip(bodies).collect())
}

fn parse_area(bytes: &[u8]) -> Result<Area, String> {
    serde_json::from_slice(bytes).map_err(|e| format!("an area doesn't read: {e}"))
}

fn parse_head(text: &str) -> Result<House, String> {
    House::parse(text)
}

/// Fetches the version `pointer` names. With `old`, only what changed for
/// what's loaded; without, the first play.
async fn prepare(
    channel: Rc<Channel>,
    pointer: Pointer,
    old: Option<(Objects, BTreeSet<String>, BTreeSet<String>)>,
) -> Result<Version, String> {
    let since = now();
    let objects = channel.objects(&pointer).await?;
    let zone = Zone::parse(&fetch_text(&url_of(&objects, icecream_zone::INDEX)?).await?)?;
    let changed = |path: &str| match &old {
        Some((o, _, _)) => objects.get(path).is_some_and(|h| o.get(path) != Some(h)),
        None => true,
    };
    let head_path = zone
        .data
        .first()
        .cloned()
        .unwrap_or_else(|| "house.json".into());
    let mut area_paths = Vec::new();
    let mut asset_paths = Vec::new();
    match &old {
        None => {
            for path in zone.first_play() {
                match zone.chunks.iter().find(|c| c.path == path).map(|c| c.kind) {
                    Some(Kind::Area) => area_paths.push(path),
                    Some(Kind::Asset) => asset_paths.push(path),
                    _ => {}
                }
            }
        }
        Some((_, areas, assets)) => {
            for chunk in &zone.chunks {
                let path = &chunk.path;
                match chunk.kind {
                    // A loaded area that changed; an area not loaded yet
                    // streams in later.
                    Kind::Area if areas.contains(path) && changed(path) => {
                        area_paths.push(path.clone());
                        for need in &chunk.needs {
                            if !assets.contains(need) && !asset_paths.contains(need) {
                                asset_paths.push(need.clone());
                            }
                        }
                    }
                    Kind::Asset if assets.contains(path) && changed(path) => {
                        if !asset_paths.contains(path) {
                            asset_paths.push(path.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    let mut paths = area_paths.clone();
    paths.extend(asset_paths.iter().cloned());
    let head_changed = changed(&head_path);
    if head_changed {
        paths.push(head_path.clone());
    }
    let mut files = fetch_paths(&objects, &paths).await?;
    let head = if head_changed {
        let (_, bytes) = files.pop().ok_or("no house settings")?;
        Some(parse_head(&String::from_utf8_lossy(&bytes))?)
    } else {
        None
    };
    let asset_files = files.split_off(area_paths.len());
    let areas = files
        .iter()
        .map(|(_, b)| parse_area(b))
        .collect::<Result<Vec<_>, _>>()?;
    let removed = match &old {
        Some((_, loaded, _)) => loaded
            .iter()
            .filter(|p| !zone.chunks.iter().any(|c| &c.path == *p))
            .filter_map(|p| {
                p.rsplit('/')
                    .next()
                    .map(|n| n.trim_end_matches(".json").to_owned())
            })
            .collect(),
        None => Vec::new(),
    };
    let mut urls: Vec<String> = paths
        .iter()
        .filter_map(|p| url_of(&objects, p).ok())
        .collect();
    urls.push(format!("/_manifest/{}", pointer.manifest));
    urls.push(url_of(&objects, icecream_zone::INDEX)?);
    Ok(Version {
        manifest: pointer.manifest,
        seq: pointer.seq,
        objects,
        zone,
        head,
        areas,
        removed,
        assets: decode_assets(asset_files)?,
        asset_paths,
        area_paths,
        urls,
        since,
    })
}

impl Live {
    /// Joins `tenant`'s channel and fetches the first play.
    ///
    /// # Errors
    /// The host can't be reached, or the version doesn't load.
    pub async fn boot(tenant: String) -> Result<(Self, Version), String> {
        live_web::keep_resource_timing();
        let (channel, pointer) = Channel::open(&tenant).await?;
        let channel = Rc::new(channel);
        let mut version = prepare(channel.clone(), pointer, None).await?;
        let head = version
            .head
            .take()
            .ok_or("the zone has no house settings")?;
        let mut live = Self {
            channel,
            tenant,
            manifest: String::new(),
            seq: 0,
            objects: Objects::new(),
            zone: version.zone.clone(),
            head: head.clone(),
            areas: BTreeMap::new(),
            assets: BTreeSet::new(),
            requested: BTreeSet::new(),
            inbox: Rc::new(RefCell::new(Inbox::default())),
        };
        version.head = Some(head);
        live.adopt(&version);
        Ok((live, version))
    }

    /// The house as loaded so far: the global settings and every area that
    /// has arrived, in the zone's order.
    #[must_use]
    pub fn house(&self) -> House {
        let mut house = self.head.clone();
        house.areas = self
            .zone
            .chunks
            .iter()
            .filter(|c| c.kind == Kind::Area)
            .filter_map(|c| self.areas.get(&c.path).cloned())
            .collect();
        house
    }

    /// Starts fetching what the next pointer brings, and the next areas
    /// by distance from `at`. Call once a frame.
    pub fn pump(&mut self, at: [f32; 2]) {
        let next = {
            let mut inbox = self.inbox.borrow_mut();
            if inbox.fetching {
                None
            } else {
                let p = self.channel.latest();
                if p.is_some() {
                    inbox.fetching = true;
                }
                p
            }
        };
        if let Some(pointer) = next {
            let old = Some((
                self.objects.clone(),
                self.areas.keys().cloned().collect(),
                self.assets.clone(),
            ));
            let inbox = self.inbox.clone();
            let channel = self.channel.clone();
            spawn_local(async move {
                let ready = prepare(channel, pointer, old).await;
                let mut inbox = inbox.borrow_mut();
                inbox.ready.push(ready);
                inbox.fetching = false;
            });
        }
        let slots = STREAMING.saturating_sub(self.inbox.borrow().in_flight);
        for path in self
            .zone
            .stream_order(at, &self.requested)
            .into_iter()
            .take(slots)
        {
            let Some(hash) = self.objects.get(&path).cloned() else {
                continue;
            };
            let needs: Vec<String> = self
                .zone
                .needs_of(&path, &self.assets)
                .into_iter()
                .filter(|n| !self.requested.contains(n))
                .collect();
            self.requested.insert(path.clone());
            self.requested.extend(needs.iter().cloned());
            self.inbox.borrow_mut().in_flight += 1;
            let inbox = self.inbox.clone();
            let objects = self.objects.clone();
            spawn_local(async move {
                let mut paths = vec![path.clone()];
                paths.extend(needs.iter().cloned());
                let result = async {
                    let mut files = fetch_paths(&objects, &paths).await?;
                    let rest = files.split_off(1);
                    let area = parse_area(&files[0].1)?;
                    Ok::<_, String>((area, decode_assets(rest)?))
                }
                .await;
                let mut inbox = inbox.borrow_mut();
                inbox.in_flight -= 1;
                match result {
                    Ok((area, assets)) => inbox.streamed.push((path, hash, area, assets, needs)),
                    Err(error) => web_sys::console::warn_1(&error.into()),
                }
            });
        }
    }

    /// Areas that streamed in, with their assets. An area whose hash moved
    /// on while it loaded is asked for again.
    pub fn take_streamed(&mut self) -> Vec<(Area, Assets)> {
        let arrived = std::mem::take(&mut self.inbox.borrow_mut().streamed);
        let mut out = Vec::new();
        for (path, hash, area, assets, needs) in arrived {
            if self.objects.get(&path) != Some(&hash) {
                self.requested.remove(&path);
                continue;
            }
            self.areas.insert(path, area.clone());
            self.assets.extend(needs);
            out.push((area, assets));
        }
        out
    }

    /// A version ready to apply, or one that failed to load.
    pub fn take_ready(&mut self) -> Option<Result<Version, String>> {
        let mut inbox = self.inbox.borrow_mut();
        (!inbox.ready.is_empty()).then(|| inbox.ready.remove(0))
    }

    /// Makes `version` the page's: its areas, settings and assets.
    pub fn adopt(&mut self, version: &Version) {
        if let Some(head) = &version.head {
            self.head = head.clone();
            self.head.areas.clear();
        }
        for (path, area) in version.area_paths.iter().zip(&version.areas) {
            self.areas.insert(path.clone(), area.clone());
            self.requested.insert(path.clone());
        }
        self.areas
            .retain(|path, _| version.zone.chunks.iter().any(|c| &c.path == path));
        self.assets.extend(version.asset_paths.iter().cloned());
        self.requested.extend(version.asset_paths.iter().cloned());
        self.manifest = version.manifest.clone();
        self.seq = version.seq;
        self.objects = version.objects.clone();
        self.zone = version.zone.clone();
    }

    /// Acks the version this page now shows.
    pub fn ack(&self, kind: &str, note: serde_json::Value) {
        live_web::ack(&self.tenant, &self.manifest, self.seq, kind, note);
    }
}
