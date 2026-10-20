//! Image input types and DeepSeek V4.1 image token counts.
//!
//! A caller supplies images as [`ImageSource`] values. Resolving an image
//! source produces an [`ImageInfo`] that holds the encoded image bytes sent to
//! the backend. The prompt holds one placeholder for each image, in the order
//! the images appear in the conversation.

use serde::{Deserialize, Serialize};

pub mod token_spec;

pub use token_spec::{CalcResizeError, ImageTokenSpec, ResizeResult};

/// Token ID of the image token in the V4.1 tokenizer. Overriding
/// [`SpecialToken::Image`] changes the image token's spelling but not this ID.
///
/// [`SpecialToken::Image`]: crate::special_tokens::SpecialToken::Image
pub const IMAGE_SPECIAL_TOKEN_ID: u32 = 129264;

/// Maximum UTF-8 byte length of an external image URL during conversion.
pub const MAX_IMAGE_URL_LEN: usize = 8192;

/// Error message for an image whose bytes are missing, undecodable, or of an
/// unsupported format. Conversion reports it for a data URL with an
/// unsupported media type, and image resolution reports it, prefixed with the
/// image's request location, for an image that cannot be preprocessed.
pub const UNSUPPORTED_IMAGE_ERROR_MESSAGE: &str = "You have uploaded an unsupported image. \
    Please make sure your image is valid and has one of the following formats: webp, png, \
    jpeg, and gif.";

/// Media type accepted for image input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageMediaType {
    Jpeg,
    Png,
    Gif,
    Webp,
}

impl ImageMediaType {
    /// Return the MIME type string, such as `image/png`.
    pub const fn mime(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
        }
    }

    /// Parse a MIME type string. Parameters after `;` are ignored, matching is
    /// case-insensitive, and `image/jpg` is accepted as an alias of
    /// `image/jpeg`. Returns `None` for an unsupported media type.
    pub fn from_mime(mime: &str) -> Option<Self> {
        let mime = mime.split(';').next().unwrap_or_default().trim();
        let (kind, subtype) = mime.split_once('/')?;
        if !kind.eq_ignore_ascii_case("image") {
            return None;
        }
        if subtype.eq_ignore_ascii_case("jpeg") || subtype.eq_ignore_ascii_case("jpg") {
            return Some(Self::Jpeg);
        }
        if subtype.eq_ignore_ascii_case("png") {
            return Some(Self::Png);
        }
        if subtype.eq_ignore_ascii_case("gif") {
            return Some(Self::Gif);
        }
        if subtype.eq_ignore_ascii_case("webp") {
            return Some(Self::Webp);
        }
        None
    }
}

/// Requested image detail level.
///
/// The OpenCV preprocessor processes [`High`](Self::High),
/// [`Original`](Self::Original), and [`Auto`](Self::Auto) identically; only
/// [`Low`](Self::Low) applies an additional long-side limit before token-budget
/// fitting. Token-budget fitting, which can resize the image, and WebP
/// encoding apply to every level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageDetail {
    Low,
    #[default]
    High,
    Original,
    Auto,
}

impl ImageDetail {
    /// Return whether the additional low-detail long-side limit applies.
    pub const fn is_low(self) -> bool {
        matches!(self, Self::Low)
    }
}

/// One image supplied with a conversation, before resolution.
///
/// Every variant records the `path` of the image: its request location as
/// error messages report it, such as `messages.0.content[1]`. Conversion fills
/// the path of each image source with the location of the message or block
/// that supplied the image; image resolution prefixes its errors with it. A
/// caller that constructs a source outside protocol conversion leaves the path
/// empty, and resolution errors then carry no location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageSource {
    /// Data URL with a base64 or percent-encoded body.
    DataUrl {
        data_url: String,
        detail: ImageDetail,
        /// Request location of the image; empty when unknown.
        path: String,
    },
    /// External image URL. Conversion and resolution check the case-insensitive
    /// `http` prefix; the fetcher is responsible for validating the full URL.
    Url {
        url: String,
        detail: ImageDetail,
        /// Request location of the image; empty when unknown.
        path: String,
    },
    /// Encoded image bytes supplied by the caller.
    Bytes {
        data: Vec<u8>,
        detail: ImageDetail,
        /// Request location of the image; empty when unknown.
        path: String,
    },
}

impl ImageSource {
    /// Return the requested detail level.
    pub const fn detail(&self) -> ImageDetail {
        match self {
            Self::DataUrl { detail, .. }
            | Self::Url { detail, .. }
            | Self::Bytes { detail, .. } => *detail,
        }
    }

    /// Return the request location of the image as error messages report it,
    /// or an empty string when the source was constructed without one.
    pub fn path(&self) -> &str {
        match self {
            Self::DataUrl { path, .. } | Self::Url { path, .. } | Self::Bytes { path, .. } => path,
        }
    }
}

/// Return whether a value is an external image URL.
///
/// Checks whether the first four bytes spell `http`, ignoring ASCII case.
/// This classifies external image references during conversion and resolution;
/// it does not validate URL syntax, the complete scheme, or the destination.
pub fn is_http_url(url: &str) -> bool {
    url.get(..4)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("http"))
}

/// A preprocessed image sent to the inference backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageInfo {
    /// Encoded image bytes.
    pub data: Vec<u8>,
    /// Width in pixels after preprocessing.
    pub width: u32,
    /// Height in pixels after preprocessing.
    pub height: u32,
}

/// Images attached to one conversation request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MultiModalData {
    /// Images in the order their placeholders appear in the prompt.
    pub images: Vec<ImageInfo>,
}

impl MultiModalData {
    /// Return whether no image is attached.
    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }

    /// Return the number of inference input tokens beyond one per
    /// placeholder, using the V4.1 image token specification.
    ///
    /// Every image contributes one placeholder to the prompt and its image
    /// token count, [`ImageTokenSpec::calc_token_count`], to the inference input.
    /// The adjustment is the sum of the differences over all images.
    ///
    /// # Errors
    ///
    /// Returns [`CalcResizeError`] when fitting an image does not converge.
    pub fn image_token_adjustment(&self) -> Result<usize, CalcResizeError> {
        let spec = ImageTokenSpec::v41();
        let mut adjustment = 0usize;
        for image in &self.images {
            adjustment += spec.calc_token_count(image.width as usize, image.height as usize)? - 1;
        }
        Ok(adjustment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_adjustment_counts_one_placeholder_per_image() {
        let spec = ImageTokenSpec::v41();
        let data = MultiModalData {
            images: vec![
                ImageInfo {
                    data: Vec::new(),
                    width: 1920,
                    height: 1080,
                },
                ImageInfo {
                    data: Vec::new(),
                    width: 640,
                    height: 480,
                },
            ],
        };
        let expected = spec.calc_token_count(1920, 1080).unwrap()
            + spec.calc_token_count(640, 480).unwrap()
            - 2;
        assert_eq!(data.image_token_adjustment().unwrap(), expected);
        assert!(MultiModalData::default().image_token_adjustment().unwrap() == 0);
    }
}
