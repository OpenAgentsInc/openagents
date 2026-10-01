//! Local image drafts shared by chat surfaces. A transport must explicitly
//! advertise image support before these bytes can leave the draft.
//!
//! The hosted conversation never carries them. A send whose draft holds
//! images sends only its words to the router and binds the images to that
//! message ([`Drafts::bind`]); they stay in the draft, and leave the device
//! only with a Coder start ([`Drafts::uploads`]). The reply to that message
//! decides ([`Drafts::settle`]): a coding reply leaves them for the start it
//! leads to, at once or by **Run Coder**; any other reply keeps them in the
//! draft and says so ([`ONLY_TO_CODER`]).
use std::{collections::BTreeMap, io::Cursor, sync::Arc};

pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_DRAFT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_IMAGES: usize = 4;
pub const MAX_DIMENSION: u32 = 4096;

#[derive(Clone, Debug)]
pub struct Image {
    pub id: String,
    pub name: String,
    pub mime: &'static str,
    pub width: u32,
    pub height: u32,
    pub bytes: Arc<Vec<u8>>,
    pub preview_width: u32,
    pub preview_height: u32,
    pub preview: Arc<Vec<u8>>,
}
impl Image {
    /// Decode and thumbnail on the input worker, never on the rendering thread.
    pub fn decode(name: &str, bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err("Images must be 8 MiB or smaller.".into());
        }
        let format = image::guess_format(&bytes).map_err(|_| "Choose a PNG or JPEG image.")?;
        let mime = match format {
            image::ImageFormat::Png => "image/png",
            image::ImageFormat::Jpeg => "image/jpeg",
            _ => {
                return Err(
                    "Choose a PNG or JPEG image. Other image formats aren't supported yet.".into(),
                );
            }
        };
        let mut reader = image::ImageReader::with_format(Cursor::new(&bytes), format);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_DIMENSION);
        limits.max_image_height = Some(MAX_DIMENSION);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let decoded = reader.decode().map_err(
            |_| "Couldn't decode this image. Choose a valid PNG or JPEG up to 4096 × 4096 pixels.",
        )?;
        let width = decoded.width();
        let height = decoded.height();
        let preview = decoded
            .thumbnail(width.min(96), height.min(60))
            .into_rgba8();
        let name: String = name
            .chars()
            .filter(|ch| !ch.is_control())
            .take(96)
            .collect();
        Ok(Self {
            id: uuid::Uuid::new_v4().simple().to_string(),
            name: if name.is_empty() {
                "Image".into()
            } else {
                name
            },
            mime,
            width,
            height,
            bytes: Arc::new(bytes),
            preview_width: preview.width(),
            preview_height: preview.height(),
            preview: Arc::new(preview.into_raw()),
        })
    }
    /// Clipboard pixels are encoded in memory; no temporary file is required.
    pub fn pixels(width: u32, height: u32, bytes: Vec<u8>) -> Result<Self, String> {
        if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
            return Err("Clipboard images must be up to 4096 × 4096 pixels.".into());
        }
        let image = image::RgbaImage::from_raw(width, height, bytes)
            .ok_or("The clipboard image has invalid pixels.")?;
        let mut encoded = Cursor::new(vec![]);
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut encoded, image::ImageFormat::Png)
            .map_err(|_| "Couldn't read the clipboard image.")?;
        Self::decode("Clipboard image", encoded.into_inner())
    }
}

/// The line a send with images shows: only the words went to the router.
pub const HELD_FOR_CODER: &str =
    "Sent your message without the images. They stay here and go to Coder if it starts.";
/// The line a reply that does not lead to Coder shows for the images bound
/// to the message it answers.
pub const ONLY_TO_CODER: &str =
    "Images go only to Coder, and this reply didn't start it. They stay in your draft.";
/// The refusal on a route that carries words only: a message to a running
/// Coder task or a computer's own thread, and the host's Run Coder handoff.
pub const TEXT_ONLY_ROUTE: &str =
    "This message can't carry images. Remove them to send it; images go to Coder when it starts.";

/// What the reply to a message with bound images decided ([`Drafts::settle`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Settled {
    /// The reply leads to Coder: the images wait in the draft for the start
    /// (at once, or **Run Coder**), which carries them.
    Coder,
    /// The reply does not: the images stay in the draft; show
    /// [`ONLY_TO_CODER`].
    Kept,
}

#[derive(Default)]
pub struct Drafts {
    images: BTreeMap<String, Vec<Image>>,
    /// The sent message (its request ID) each conversation's draft images
    /// are bound to, until its reply settles them.
    bound: BTreeMap<String, String>,
}
impl Drafts {
    pub fn get(&self, chat: &str) -> &[Image] {
        self.images.get(chat).map_or(&[], Vec::as_slice)
    }
    pub fn add(&mut self, chat: &str, image: Image) -> Result<(), String> {
        if self.get(chat).len() >= MAX_IMAGES {
            return Err("A draft can hold up to four images.".into());
        }
        let bytes: usize = self
            .images
            .values()
            .flatten()
            .map(|image| image.bytes.len())
            .sum();
        if bytes + image.bytes.len() > MAX_DRAFT_BYTES {
            return Err(
                "Image drafts can hold 16 MiB in total. Remove an image before adding another."
                    .into(),
            );
        }
        self.images.entry(chat.into()).or_default().push(image);
        Ok(())
    }
    pub fn remove(&mut self, chat: &str, id: &str) {
        if let Some(images) = self.images.get_mut(chat) {
            images.retain(|image| image.id != id);
        }
        if self.get(chat).is_empty() {
            self.images.remove(chat);
            self.bound.remove(chat);
        }
    }
    /// The refusal for a route that carries words only, while the draft
    /// holds images ([`TEXT_ONLY_ROUTE`]). A message to the router is not
    /// one: it sends its words and binds the images ([`Drafts::bind`]).
    pub fn text_only_refusal(&self, chat: &str) -> Option<&'static str> {
        (!self.get(chat).is_empty()).then_some(TEXT_ONLY_ROUTE)
    }
    /// Bind the draft's images to the message `request` just sent to the
    /// router with only its words; `true` when there were images to bind.
    /// A later send binds them to that message instead.
    pub fn bind(&mut self, chat: &str, request: &str) -> bool {
        if self.get(chat).is_empty() {
            return false;
        }
        self.bound.insert(chat.into(), request.into());
        true
    }
    /// The message the draft's images are bound to, if any.
    pub fn bound(&self, chat: &str) -> Option<&str> {
        self.bound.get(chat).map(String::as_str)
    }
    /// The conversations whose images are bound to a sent message.
    pub fn bound_chats(&self) -> Vec<String> {
        self.bound.keys().cloned().collect()
    }
    /// Move a draft, with its binding, to another conversation key (a new
    /// chat's draft once the chat has its ID).
    pub fn rebind(&mut self, from: &str, to: &str) {
        if from == to {
            return;
        }
        if let Some(images) = self.images.remove(from) {
            self.images.insert(to.into(), images);
        }
        if let Some(request) = self.bound.remove(from) {
            self.bound.insert(to.into(), request);
        }
    }
    /// Read the reply to the message the draft's images are bound to.
    /// `None` while there is no binding or no finished reply yet. A reply
    /// that offers Coder (the router's typed offer, or the computer lane;
    /// [`openagents_chat::delegation::offered`]) settles them for Coder; any
    /// other finished reply, or a stopped one, keeps them in the draft.
    /// Either way the binding ends; the images stay in the draft until a
    /// Coder start is accepted ([`Drafts::clear`]) or the person removes
    /// them.
    pub fn settle(
        &mut self,
        chat: &str,
        turns: &[openagents_chat::basic_coder::Turn],
        busy: bool,
        computer_lane: bool,
    ) -> Option<Settled> {
        use openagents_chat::basic_coder::Role;
        let request = self.bound.get(chat)?;
        if busy {
            return None;
        }
        let at = turns
            .iter()
            .rposition(|turn| turn.request.as_deref() == Some(request.as_str()))?;
        let reply = turns.get(at + 1)?;
        if reply.role != Role::Assistant {
            return None;
        }
        self.bound.remove(chat);
        if self.get(chat).is_empty() {
            return None;
        }
        let coder = !reply.stopped
            && openagents_chat::delegation::offered(reply.meta.as_ref(), computer_lane);
        Some(if coder { Settled::Coder } else { Settled::Kept })
    }
    /// The draft's images as a Coder task carries them to the computer that
    /// runs it: the exact bytes and the digest each is named by. Phone and
    /// desktop both send what this returns. A refusal names the image that
    /// cannot go, before anything is sent; the draft is unchanged either way.
    pub fn uploads(&self, chat: &str) -> Result<Vec<Upload>, String> {
        self.get(chat)
            .iter()
            .map(|image| {
                Upload::new(&image.name, image.bytes.clone()).map_err(|error| error.message)
            })
            .collect()
    }
    /// Drop the draft's images once the run that carries them is accepted.
    pub fn clear(&mut self, chat: &str) {
        self.images.remove(chat);
        self.bound.remove(chat);
    }
}

/// One image on its way to a Coder task ([`Drafts::uploads`]).
pub use coder_host::access::media::Upload;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jpeg_and_large_image_previews_are_bounded() {
        let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            1200,
            800,
            image::Rgb([20, 60, 90]),
        ));
        let mut jpeg = Cursor::new(vec![]);
        image.write_to(&mut jpeg, image::ImageFormat::Jpeg).unwrap();
        let decoded = Image::decode("Photo.jpg", jpeg.into_inner()).unwrap();
        assert_eq!(decoded.mime, "image/jpeg");
        assert!(decoded.preview_width <= 96 && decoded.preview_height <= 60);
        assert_eq!((decoded.width, decoded.height), (1200, 800));
        let mut drafts = Drafts::default();
        let mut image = Image::pixels(1, 1, vec![0; 4]).unwrap();
        image.bytes = Arc::new(vec![0; MAX_IMAGE_BYTES]);
        drafts.add("a", image.clone()).unwrap();
        drafts.add("b", image.clone()).unwrap();
        assert!(drafts.add("c", image).is_err());
    }
    #[test]
    fn validates_previews_and_preserves_drafts_when_the_route_is_text_only() {
        let image = Image::pixels(2, 1, vec![255, 0, 0, 255, 0, 255, 0, 255]).unwrap();
        assert_eq!((image.width, image.height), (2, 1));
        assert_eq!(image.preview.len(), 8);
        assert_eq!(image.mime, "image/png");
        let mut drafts = Drafts::default();
        let id = image.id.clone();
        drafts.add("first", image).unwrap();
        assert!(drafts.text_only_refusal("first").is_some());
        assert!(drafts.get("second").is_empty());
        assert_eq!(drafts.get("first")[0].id, id);
        drafts.remove("first", &id);
        assert!(drafts.text_only_refusal("first").is_none());
        assert!(Image::decode("bad", b"not an image".to_vec()).is_err());
        assert!(Image::pixels(4097, 1, vec![]).is_err());
        assert!(Image::pixels(2, 1, vec![]).is_err());
        assert!(Image::decode("large", vec![0; MAX_IMAGE_BYTES + 1]).is_err());
    }
    #[test]
    fn a_coding_route_carries_the_exact_draft_bytes_and_keeps_the_draft() {
        let mut drafts = Drafts::default();
        let image = Image::pixels(3, 2, vec![9; 24]).unwrap();
        let bytes = image.bytes.clone();
        drafts.add("talk:a", image).unwrap();
        let uploads = drafts.uploads("talk:a").unwrap();
        assert_eq!(uploads.len(), 1);
        assert_eq!(*uploads[0].bytes, *bytes);
        assert_eq!(uploads[0].reference.media_type, "image/png");
        assert_eq!(
            uploads[0].reference.digest,
            coder_host::access::media::digest(&bytes)
        );
        // Building uploads sends nothing and keeps the draft; a words-only
        // route still refuses.
        assert_eq!(drafts.get("talk:a").len(), 1);
        assert!(drafts.text_only_refusal("talk:a").is_some());
        drafts.clear("talk:a");
        assert!(drafts.get("talk:a").is_empty());
    }

    fn turn(
        role: openagents_chat::basic_coder::Role,
        request: Option<&str>,
        offers: Vec<openagents_chat::router::Offer>,
    ) -> openagents_chat::basic_coder::Turn {
        use openagents_chat::basic_coder::{Role, Turn};
        let mut turn = match role {
            Role::User => Turn::user("fix this layout bug"),
            Role::Assistant => Turn::assistant("On it.", None),
        };
        turn.request = request.map(str::to_owned);
        if !offers.is_empty() {
            turn.meta = Some(openagents_chat::router::Meta {
                offers,
                ..Default::default()
            });
        }
        turn
    }

    /// A send binds the draft's images to its message; the reply settles
    /// them: a coding reply leaves them for Coder's start, any other keeps
    /// them in the draft, and a draft without images binds nothing.
    #[test]
    fn a_reply_settles_the_images_bound_to_its_message() {
        use openagents_chat::basic_coder::Role;
        use openagents_chat::router::Offer;
        let mut drafts = Drafts::default();
        assert!(!drafts.bind("new", "r0"));
        drafts
            .add("new", Image::pixels(3, 2, vec![9; 24]).unwrap())
            .unwrap();
        assert!(drafts.bind("new", "r1"));
        drafts.rebind("new", "talk:a");
        assert!(drafts.get("new").is_empty());
        assert_eq!(drafts.bound("talk:a"), Some("r1"));
        let mut turns = vec![turn(Role::User, Some("r1"), vec![])];
        // No reply yet, or one still streaming: nothing settles.
        assert_eq!(drafts.settle("talk:a", &turns, false, false), None);
        turns.push(turn(Role::Assistant, None, vec![Offer::RunCoder]));
        assert_eq!(drafts.settle("talk:a", &turns, true, false), None);
        assert_eq!(
            drafts.settle("talk:a", &turns, false, false),
            Some(Settled::Coder)
        );
        // Settled once; the images wait in the draft for the start.
        assert_eq!(drafts.settle("talk:a", &turns, false, false), None);
        assert_eq!(drafts.get("talk:a").len(), 1);
        // A reply with no Coder offer keeps them and says so.
        assert!(drafts.bind("talk:a", "r2"));
        turns.push(turn(Role::User, Some("r2"), vec![]));
        turns.push(turn(Role::Assistant, None, vec![]));
        assert_eq!(
            drafts.settle("talk:a", &turns, false, false),
            Some(Settled::Kept)
        );
        assert_eq!(drafts.get("talk:a").len(), 1);
        // The computer lane alone is not a coding route: that reply
        // answered (#10079).
        assert!(drafts.bind("talk:a", "r2"));
        assert_eq!(
            drafts.settle("talk:a", &turns, false, true),
            Some(Settled::Kept)
        );
        // A dispatch-routed reply on the computer lane is.
        turns.last_mut().unwrap().meta = Some(openagents_chat::router::Meta {
            route: Some(openagents_chat::delegation::DISPATCH_ROUTE.into()),
            ..Default::default()
        });
        assert!(drafts.bind("talk:a", "r2"));
        assert_eq!(
            drafts.settle("talk:a", &turns, false, true),
            Some(Settled::Coder)
        );
        // An accepted start lets the images and the binding go.
        assert!(drafts.bind("talk:a", "r2"));
        drafts.clear("talk:a");
        assert_eq!(drafts.bound("talk:a"), None);
    }

    #[test]
    fn bounds_image_count_across_each_conversation() {
        let mut drafts = Drafts::default();
        for _ in 0..MAX_IMAGES {
            drafts
                .add("a", Image::pixels(1, 1, vec![0; 4]).unwrap())
                .unwrap();
        }
        assert!(
            drafts
                .add("a", Image::pixels(1, 1, vec![0; 4]).unwrap())
                .is_err()
        );
        drafts
            .add("b", Image::pixels(1, 1, vec![0; 4]).unwrap())
            .unwrap();
        assert_eq!(drafts.get("a").len(), 4);
        assert_eq!(drafts.get("b").len(), 1);
    }
}
