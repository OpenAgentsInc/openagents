//! Resource bytes stay with the task owner. Callers supply references, not paths.

use super::{Error, Store, local, media, pending};
use route_contract::digest_of;
pub use route_contract::resources::*;
use serde::{Deserialize, Serialize};
use std::{
    io::Read as _,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

#[derive(Clone)]
pub struct Service {
    store: PathBuf,
    task: String,
    agent: Option<pending::Service>,
}
impl Service {
    pub fn for_agent(store: &Path, agent: pending::Service) -> Self {
        Self {
            store: store.into(),
            task: agent.scope().task.clone(),
            agent: Some(agent),
        }
    }
    pub fn for_viewer(store: &Path, task: &str) -> Result<Self, Error> {
        local::record(store, task).ok_or(Error::NotFound)?;
        Store::open(store)?.show(task)?;
        Ok(Self {
            store: store.into(),
            task: task.into(),
            agent: None,
        })
    }
    pub fn input_references(&self) -> Result<Vec<Reference>, Error> {
        let task = Store::open(&self.store)?.show(&self.task)?;
        let mut references: Vec<Reference> = task
            .intent
            .images
            .iter()
            .map(|reference| {
                let bytes = media::load(&self.store, &self.task, reference)?;
                initial_reference(&self.task, reference, &bytes)
            })
            .collect::<Result<_, Error>>()?;
        if let Some(input) = task
            .follow_ups
            .last()
            .and_then(|follow| follow.input.as_ref())
        {
            references.extend(input.resources.clone());
        }
        Ok(references)
    }
    pub fn read(
        &self,
        reference: &Reference,
        usage: Usage,
    ) -> Result<coder_sdk::resources::Bound, Error> {
        if let Some(agent) = &self.agent {
            agent.with_active(|| self.perform(reference, usage))
        } else {
            self.perform(reference, usage)
        }
    }
    fn perform(
        &self,
        reference: &Reference,
        usage: Usage,
    ) -> Result<coder_sdk::resources::Bound, Error> {
        if !reference.allows(usage)
            || reference.owner.host != local::LOCAL_HOST
            || reference.owner.task != self.task
        {
            return Err(Error::InvalidTransition);
        }
        let task = Store::open(&self.store)?.show(&self.task)?;
        let bytes = match &reference.origin {
            Origin::Initial => {
                let image = task
                    .intent
                    .images
                    .iter()
                    .find(|image| {
                        image.digest == reference.digest.as_str()
                            && image.media_type == reference.media_type.as_str()
                            && image.size == reference.size as u64
                            && route_contract::resources::name(&image.name) == reference.name
                    })
                    .ok_or(Error::NotFound)?;
                let bytes = media::load(&self.store, &self.task, image)?;
                if initial_reference(&self.task, image, &bytes)? != *reference {
                    return Err(Error::InvalidTransition);
                }
                bytes
            }
            Origin::Input { .. } => {
                let known = task
                    .follow_ups
                    .iter()
                    .filter_map(|follow| follow.input.as_ref().map(|input| (follow, input)))
                    .any(|(follow, input)| {
                        input.valid_for(&self.task, &follow.prompt)
                            && input.resources.contains(reference)
                    });
                if !known {
                    return Err(Error::NotFound);
                }
                media::load(&self.store, &self.task, &image_ref(reference))?
            }
            Origin::Produced { .. } => {
                let (directory, book) = read_book(&self.store, &self.task)?;
                if !book.resources.contains(reference) {
                    return Err(Error::NotFound);
                }
                let mut bytes = vec![];
                super::private_open(&data_path(&directory, &reference.digest), false, false)?
                    .take(MAX_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)?;
                bytes
            }
        };
        coder_sdk::resources::Bound::new(reference.clone(), Arc::new(bytes))
            .map_err(|_| Error::Corrupt("resource bytes differ from their reference"))
    }
    /// Publish bytes the running host already has. No path or URI is opened.
    pub fn publish(
        &self,
        item: &str,
        name: &str,
        media_type: MediaType,
        bytes: &[u8],
    ) -> Result<Reference, Error> {
        let agent = self.agent.as_ref().ok_or(Error::InvalidTransition)?;
        agent.with_active(|| {
            let reference = reference(
                Owner {
                    host: agent.scope().host.clone(),
                    task: self.task.clone(),
                },
                Origin::Produced {
                    scope: agent.scope().clone(),
                    item: item.into(),
                },
                name,
                media_type,
                bytes,
                vec![Usage::Preview, Usage::Read],
            )?;
            let directory = directory(&self.store, &self.task)?;
            let lock = super::open_lock(&directory.join("book.lock"))?;
            super::take_lock(&lock, Duration::from_secs(1))?;
            super::verify_same_file(&directory.join("book.lock"), &lock)?;
            let (_, mut book) = read_book(&self.store, &self.task)?;
            let known = book.resources.contains(&reference);
            if !known
                && (book.resources.len() >= 64
                    || book
                        .resources
                        .iter()
                        .map(|resource| resource.size)
                        .sum::<usize>()
                        + bytes.len()
                        > MAX_TASK_BYTES)
            {
                return Err(Error::LimitExceeded);
            }
            let path = data_path(&directory, &reference.digest);
            let mut stored = 0u64;
            for (count, entry) in std::fs::read_dir(&directory)?.enumerate() {
                if count >= 256 {
                    return Err(Error::LimitExceeded);
                }
                let entry = entry?;
                super::regular_or_absent(&entry.path())?;
                stored = stored
                    .checked_add(entry.metadata()?.len())
                    .ok_or(Error::LimitExceeded)?;
            }
            let replacing = if super::regular_or_absent(&path)? {
                path.metadata()?.len()
            } else {
                0
            };
            if stored
                .saturating_sub(replacing)
                .saturating_add(bytes.len() as u64)
                > (MAX_TASK_BYTES + 2 * 1024 * 1024) as u64
            {
                return Err(Error::LimitExceeded);
            }
            if super::regular_or_absent(&path)? {
                let mut current = vec![];
                super::private_open(&path, false, false)?
                    .take(MAX_BYTES as u64 + 1)
                    .read_to_end(&mut current)?;
                if current != bytes {
                    super::replace_file(
                        &directory,
                        path.file_name().unwrap().to_str().unwrap(),
                        bytes,
                    )?;
                }
            } else {
                super::write_new(&path, bytes)?;
            }
            super::sync_directory(&directory)?;
            if known {
                return Ok(reference);
            }
            book.resources.push(reference.clone());
            let bytes = serde_json::to_vec(&book).map_err(|_| Error::UnsupportedSchema)?;
            super::replace_file(&directory, "book.json", &bytes)?;
            Ok(reference)
        })
    }
}
impl coder_sdk::resources::Service for Service {
    fn read(
        &self,
        reference: &Reference,
        usage: Usage,
    ) -> Result<coder_sdk::resources::Bound, String> {
        self.read(reference, usage)
            .map_err(|_| "This attachment is unavailable or changed. Add it again or retry.".into())
    }
}
fn reference(
    owner: Owner,
    origin: Origin,
    name: &str,
    media_type: MediaType,
    bytes: &[u8],
    uses: Vec<Usage>,
) -> Result<Reference, Error> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err(Error::LimitExceeded);
    }
    let dimensions = if media_type.image() {
        let format = if media_type == MediaType::Png {
            image::ImageFormat::Png
        } else {
            image::ImageFormat::Jpeg
        };
        if image::guess_format(bytes).ok() != Some(format) {
            return Err(Error::UnsupportedSchema);
        }
        let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_DIMENSION);
        limits.max_image_height = Some(MAX_DIMENSION);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let image = reader.decode().map_err(|_| Error::UnsupportedSchema)?;
        Some(Dimensions {
            width: image.width(),
            height: image.height(),
        })
    } else {
        let text = std::str::from_utf8(bytes).map_err(|_| Error::UnsupportedSchema)?;
        if text.contains('\0')
            || (media_type == MediaType::Json
                && serde_json::from_str::<serde_json::Value>(text).is_err())
        {
            return Err(Error::UnsupportedSchema);
        }
        None
    };
    let mut reference = Reference {
        schema: SCHEMA.into(),
        id: digest_of(&()),
        owner,
        origin,
        digest: route_contract::Digest::of_bytes(bytes),
        media_type,
        size: bytes.len(),
        name: route_contract::resources::name(name),
        dimensions,
        uses,
    };
    reference.seal();
    if !reference.valid() {
        return Err(Error::UnsupportedSchema);
    }
    Ok(reference)
}
fn initial_reference(
    task: &str,
    image: &media::wire::ImageRef,
    bytes: &[u8],
) -> Result<Reference, Error> {
    let mut reference = image_reference(task, "initial", image, bytes)?;
    reference.origin = Origin::Initial;
    reference.seal();
    Ok(reference)
}
pub(super) fn image_reference(
    task: &str,
    message: &str,
    image: &media::wire::ImageRef,
    bytes: &[u8],
) -> Result<Reference, Error> {
    media::verify(image, bytes)?;
    let media_type = MediaType::parse(&image.media_type)
        .filter(|media_type| media_type.image())
        .ok_or(Error::UnsupportedSchema)?;
    reference(
        Owner {
            host: local::LOCAL_HOST.into(),
            task: task.into(),
        },
        Origin::Input {
            message: message.into(),
        },
        &image.name,
        media_type,
        bytes,
        vec![Usage::PromptImage, Usage::Preview, Usage::Read],
    )
}
pub(super) fn image_ref(reference: &Reference) -> media::wire::ImageRef {
    media::wire::ImageRef {
        digest: reference.digest.as_str().into(),
        media_type: reference.media_type.as_str().into(),
        size: reference.size as u64,
        name: reference.name.clone(),
    }
}
#[derive(Serialize, Deserialize)]
struct Book {
    schema: String,
    task: String,
    resources: Vec<Reference>,
}
pub(super) fn directory(store: &Path, task: &str) -> Result<PathBuf, Error> {
    let root = store.join("resources");
    super::prepare_directory(&root)?;
    let directory = root.join(digest_of(&task).as_str().trim_start_matches("sha256:"));
    super::prepare_directory(&directory)?;
    Ok(directory)
}
fn data_path(directory: &Path, digest: &route_contract::Digest) -> PathBuf {
    directory.join(digest.as_str().trim_start_matches("sha256:"))
}
fn read_book(store: &Path, task: &str) -> Result<(PathBuf, Book), Error> {
    let directory = directory(store, task)?;
    let path = directory.join("book.json");
    let book = if super::regular_or_absent(&path)? {
        let mut bytes = vec![];
        super::private_open(&path, false, false)?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 1024 * 1024 {
            return Err(Error::LimitExceeded);
        }
        let book: Book = serde_json::from_slice(&bytes).map_err(|_| Error::UnsupportedSchema)?;
        if book.schema != SCHEMA
            || book.task != task
            || book.resources.len() > 64
            || book.resources.iter().any(|resource| {
                !resource.valid()
                    || resource.owner.host != local::LOCAL_HOST
                    || resource.owner.task != task
                    || !matches!(resource.origin, Origin::Produced { .. })
            })
            || book
                .resources
                .iter()
                .map(|resource| resource.size)
                .sum::<usize>()
                > MAX_TASK_BYTES
        {
            return Err(Error::UnsupportedSchema);
        }
        book
    } else {
        Book {
            schema: SCHEMA.into(),
            task: task.into(),
            resources: vec![],
        }
    };
    Ok((directory, book))
}

impl local::Local {
    pub fn resource(
        &self,
        task: &str,
        reference: &Reference,
    ) -> Result<coder_sdk::resources::Bound, String> {
        Service::for_viewer(self.store(), task)
            .and_then(|service| service.read(reference, Usage::Preview))
            .map_err(|_| {
                "This attachment is unavailable or changed. Refresh it and try again.".into()
            })
    }
}
