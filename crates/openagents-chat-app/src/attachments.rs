//! Local image drafts shared by chat surfaces. A transport must explicitly
//! advertise image support before these bytes can leave the draft.
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

#[derive(Default)]
pub struct Drafts {
    images: BTreeMap<String, Vec<Image>>,
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
        }
    }
    /// The current hosted NIP-CJ conversation contract carries text only.
    pub fn hosted_send_refusal(&self, chat: &str) -> Option<&'static str> {
        (!self.get(chat).is_empty())
            .then_some("Hosted chat accepts text only. Remove the images to send.")
    }
}

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
        assert!(drafts.hosted_send_refusal("first").is_some());
        assert!(drafts.get("second").is_empty());
        assert_eq!(drafts.get("first")[0].id, id);
        drafts.remove("first", &id);
        assert!(drafts.hosted_send_refusal("first").is_none());
        assert!(Image::decode("bad", b"not an image".to_vec()).is_err());
        assert!(Image::pixels(4097, 1, vec![]).is_err());
        assert!(Image::pixels(2, 1, vec![]).is_err());
        assert!(Image::decode("large", vec![0; MAX_IMAGE_BYTES + 1]).is_err());
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
