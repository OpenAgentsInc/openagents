//! CPU allocations and deferred WebGL texture uploads. GPUI can call the
//! atlas through a Send + Sync trait; only the renderer touches JS objects.
use etagere::{AllocId, AtlasAllocator, size2};
use gpui::{
    AtlasKey, AtlasTextureId, AtlasTextureKind, AtlasTile, Bounds, DevicePixels, PlatformAtlas,
    Point, Size, TileId,
};
use std::{borrow::Cow, collections::HashMap, sync::Mutex};

pub(super) struct Page {
    pub size: i32,
    pub kind: AtlasTextureKind,
    allocator: AtlasAllocator,
    pub tiles: HashMap<AllocId, (AtlasTile, Vec<u8>)>,
    pub dirty: bool,
    pub generation: u64,
}

#[derive(Default)]
pub(super) struct State {
    pub pages: Vec<Option<Page>>,
    keys: HashMap<AtlasKey, AtlasTile>,
    generation: u64,
}

pub(super) struct Atlas {
    pub state: Mutex<State>,
    pub max_size: i32,
}

impl PlatformAtlas for Atlas {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> anyhow::Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> anyhow::Result<Option<AtlasTile>> {
        if let Some(tile) = self.state.lock().unwrap().keys.get(key) {
            return Ok(Some(*tile));
        }
        let Some((size, bytes)) = build()? else {
            return Ok(None);
        };
        let kind = key.texture_kind();
        let channels = if kind == AtlasTextureKind::Monochrome {
            1
        } else {
            4
        };
        let (w, h) = (size.width.0, size.height.0);
        anyhow::ensure!(
            w > 0 && h > 0 && w + 2 <= self.max_size && h + 2 <= self.max_size,
            "image exceeds the WebGL2 atlas size"
        );
        anyhow::ensure!(
            bytes.len() == w as usize * h as usize * channels,
            "wrong sprite pixel count"
        );
        let mut state = self.state.lock().unwrap();
        if let Some(tile) = state.keys.get(key) {
            return Ok(Some(*tile));
        }
        let wanted = size2(w + 2, h + 2);
        let found = state.pages.iter_mut().enumerate().find_map(|(i, page)| {
            let page = page.as_mut()?;
            (page.kind == kind)
                .then(|| page.allocator.allocate(wanted))
                .flatten()
                .map(|a| (i, a))
        });
        let (index, allocation) = found.unwrap_or_else(|| {
            let dimension = 1024.max(w + 2).max(h + 2).min(self.max_size);
            let mut allocator = AtlasAllocator::new(size2(dimension, dimension));
            let allocation = allocator.allocate(wanted).unwrap();
            let index = state
                .pages
                .iter()
                .position(Option::is_none)
                .unwrap_or(state.pages.len());
            state.generation += 1;
            let page = Some(Page {
                size: dimension,
                kind,
                allocator,
                tiles: HashMap::new(),
                dirty: true,
                generation: state.generation,
            });
            if index == state.pages.len() {
                state.pages.push(page);
            } else {
                state.pages[index] = page;
            }
            (index, allocation)
        });
        let tile = AtlasTile {
            texture_id: AtlasTextureId {
                index: index as u32,
                kind,
            },
            tile_id: TileId(allocation.id.serialize()),
            padding: 0,
            bounds: Bounds {
                origin: Point {
                    x: DevicePixels(allocation.rectangle.min.x + 1),
                    y: DevicePixels(allocation.rectangle.min.y + 1),
                },
                size,
            },
        };
        let page = state.pages[index].as_mut().unwrap();
        page.tiles.insert(allocation.id, (tile, bytes.into_owned()));
        page.dirty = true;
        state.keys.insert(key.clone(), tile);
        Ok(Some(tile))
    }

    fn remove(&self, key: &AtlasKey) {
        let mut state = self.state.lock().unwrap();
        if let Some(tile) = state.keys.remove(key) {
            let index = tile.texture_id.index as usize;
            if let Some(page) = &mut state.pages[index] {
                let id = AllocId::deserialize(tile.tile_id.0);
                page.tiles.remove(&id);
                page.allocator.deallocate(id);
                page.dirty = true;
                if page.tiles.is_empty() {
                    state.pages[index] = None;
                }
            }
        }
    }
}
