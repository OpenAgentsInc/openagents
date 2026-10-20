//! Browser-decoded images painted by GPUI. Image codecs stay in the browser.

use std::sync::Arc;

use gpui::{
    AnyElement, App, Asset, Bounds, Element, ElementId, GlobalElementId, Image, InspectorElementId,
    IntoElement, LayoutId, Length, ObjectFit, Pixels, Refineable, RenderImage, SharedUri, Style,
    StyleRefinement, Styled, Window, px,
};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

#[derive(Clone, Debug)]
pub enum Source {
    Bytes(Arc<Image>),
    Uri(SharedUri),
    Mask(&'static [u8], u32),
}

impl std::hash::Hash for Source {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Bytes(image) => {
                0u8.hash(state);
                image.id().hash(state);
            }
            Self::Uri(uri) => {
                1u8.hash(state);
                uri.hash(state);
            }
            Self::Mask(mask, size) => {
                2u8.hash(state);
                mask.hash(state);
                size.hash(state);
            }
        }
    }
}

impl From<Arc<Image>> for Source {
    fn from(image: Arc<Image>) -> Self {
        Self::Bytes(image)
    }
}
impl From<SharedUri> for Source {
    fn from(uri: SharedUri) -> Self {
        Self::Uri(uri)
    }
}

struct Decoder;
impl Asset for Decoder {
    type Source = Source;
    type Output = Result<Arc<RenderImage>, Arc<str>>;

    fn load(source: Source, _: &mut App) -> impl Future<Output = Self::Output> + Send + 'static {
        let (send, receive) = futures::channel::oneshot::channel();
        // DOM objects remain on the browser thread; only pixels cross GPUI's
        // Send boundary. GPUI caches the result and redraws its subscribers.
        wasm_bindgen_futures::spawn_local(async move {
            let result = decode(source)
                .await
                .map_err(|error| Arc::from(format!("{error:?}")));
            let _ = send.send(result);
        });
        async move {
            receive
                .await
                .unwrap_or_else(|_| Err(Arc::from("image loading stopped")))
        }
    }
}

async fn decode(source: Source) -> Result<Arc<RenderImage>, wasm_bindgen::JsValue> {
    let window = web_sys::window().unwrap();
    let blob: web_sys::Blob = match source {
        Source::Mask(_, _) => return Err("icons decode synchronously".into()),
        Source::Bytes(image) => {
            let parts = js_sys::Array::new();
            parts.push(&js_sys::Uint8Array::from(image.bytes()));
            web_sys::Blob::new_with_u8_array_sequence(&parts)?
        }
        Source::Uri(uri) => {
            let response: web_sys::Response = JsFuture::from(window.fetch_with_str(&uri))
                .await?
                .dyn_into()?;
            if !response.ok() {
                return Err(format!("image: HTTP {}", response.status()).into());
            }
            JsFuture::from(response.blob()?).await?.dyn_into()?
        }
    };
    let bitmap: web_sys::ImageBitmap = JsFuture::from(window.create_image_bitmap_with_blob(&blob)?)
        .await?
        .dyn_into()?;
    let width = bitmap.width();
    let height = bitmap.height();
    // Bound the pixel allocation before reading it back from the canvas.
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 16_777_216 {
        bitmap.close();
        return Err("image is too large".into());
    }
    let canvas: web_sys::HtmlCanvasElement = window
        .document()
        .unwrap()
        .create_element("canvas")?
        .dyn_into()?;
    canvas.set_width(width);
    canvas.set_height(height);
    let context: web_sys::CanvasRenderingContext2d =
        canvas.get_context("2d")?.unwrap().dyn_into()?;
    let drawn = context.draw_image_with_image_bitmap(&bitmap, 0., 0.);
    bitmap.close();
    drawn?;
    let mut pixels = context
        .get_image_data(0., 0., width.into(), height.into())?
        .data()
        .0;
    // GPUI's color sprites use BGRA; canvas readback is unpremultiplied RGBA.
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let pixels = image::RgbaImage::from_raw(width, height, pixels).ok_or("invalid image pixels")?;
    Ok(Arc::new(RenderImage::new([image::Frame::new(pixels)])))
}

/// An image with the browser's format support, laid out and painted by GPUI.
pub struct BrowserImage {
    source: Source,
    style: StyleRefinement,
    fit: ObjectFit,
    fallback: Option<Box<dyn Fn() -> AnyElement>>,
}

pub fn img(source: impl Into<Source>) -> BrowserImage {
    BrowserImage {
        source: source.into(),
        style: StyleRefinement::default(),
        fit: ObjectFit::Contain,
        fallback: None,
    }
}

/// An alpha mask generated from a fixed icon on the build machine.
pub fn mask_image(mask: &'static [u8], side: u32) -> BrowserImage {
    img(Source::Mask(mask, side))
}

fn tinted_mask(mask: &'static [u8], side: u32, color: gpui::Hsla) -> Arc<RenderImage> {
    type Key = (usize, u32);
    thread_local! { static CACHE: std::cell::RefCell<std::collections::HashMap<Key, Arc<RenderImage>>> = Default::default(); }
    let rgba: gpui::Rgba = color.into();
    let key = (mask.as_ptr() as usize, u32::from(rgba));
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(image) = cache.get(&key) {
            return image.clone();
        }
        let pixels: Vec<u8> = mask
            .iter()
            .flat_map(|alpha| {
                [
                    (rgba.b * 255.) as u8,
                    (rgba.g * 255.) as u8,
                    (rgba.r * 255.) as u8,
                    (*alpha as f32 * rgba.a) as u8,
                ]
            })
            .collect();
        let image = image::RgbaImage::from_raw(side, side, pixels)
            .unwrap_or_else(|| image::RgbaImage::new(1, 1));
        let image = Arc::new(RenderImage::new([image::Frame::new(image)]));
        if cache.len() >= 256 {
            cache.clear();
        }
        cache.insert(key, image.clone());
        image
    })
}

impl BrowserImage {
    pub fn object_fit(mut self, fit: ObjectFit) -> Self {
        self.fit = fit;
        self
    }
    pub fn with_fallback(mut self, fallback: impl Fn() -> AnyElement + 'static) -> Self {
        self.fallback = Some(Box::new(fallback));
        self
    }
}
impl Styled for BrowserImage {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl IntoElement for BrowserImage {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

pub struct Layout {
    style: Style,
    image: Option<Arc<RenderImage>>,
    fallback: Option<AnyElement>,
}

impl Element for BrowserImage {
    type RequestLayoutState = Layout;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Layout) {
        let mut style = Style::default();
        style.refine(&self.style);
        let loaded = match &self.source {
            Source::Mask(mask, side) => Some(Ok(tinted_mask(
                mask,
                *side,
                style.text.color.unwrap_or(window.text_style().color),
            ))),
            _ => window.use_asset::<Decoder>(&self.source, cx),
        };
        let mut fallback = match &loaded {
            Some(Err(_)) => self.fallback.as_ref().map(|fallback| fallback()),
            _ => None,
        };
        if let Some(mut fallback) = fallback.take() {
            let id = fallback.request_layout(window, cx);
            return (
                id,
                Layout {
                    style,
                    image: None,
                    fallback: Some(fallback),
                },
            );
        }
        let image = loaded.and_then(Result::ok);
        if let Some(image) = &image {
            let size = image.size(0).map(|dimension| px(dimension.0 as f32));
            if matches!(style.size.width, Length::Auto) || matches!(style.size.height, Length::Auto)
            {
                style.aspect_ratio = Some(size.width / size.height);
            }
            if matches!(style.size.width, Length::Auto) {
                let width = match style.size.height {
                    Length::Definite(gpui::DefiniteLength::Absolute(height)) => {
                        size.width * (height.to_pixels(window.rem_size()) / size.height)
                    }
                    _ => size.width,
                };
                style.size.width = width.into();
            }
            if matches!(style.size.height, Length::Auto) {
                let height = match style.size.width {
                    Length::Definite(gpui::DefiniteLength::Absolute(width)) => {
                        size.height * (width.to_pixels(window.rem_size()) / size.width)
                    }
                    _ => size.height,
                };
                style.size.height = height.into();
            }
        }
        let id = window.request_layout(style.clone(), [], cx);
        (
            id,
            Layout {
                style,
                image,
                fallback: None,
            },
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        layout: &mut Layout,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(fallback) = &mut layout.fallback {
            fallback.prepaint(window, cx);
        }
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Layout,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(fallback) = &mut layout.fallback {
            fallback.paint(window, cx);
            return;
        }
        let Some(image) = &layout.image else {
            return;
        };
        let fitted = self.fit.get_bounds(bounds, image.size(0));
        let visible = bounds.intersect(&fitted);
        let corners = layout
            .style
            .corner_radii
            .to_pixels(window.rem_size())
            .clamp_radii_for_quad_size(visible.size);
        layout.style.paint(bounds, window, cx, |window, _| {
            if let Err(error) =
                window.paint_image_fitted(visible, fitted, corners, image.clone(), 0, false)
            {
                log::error!("image: {error}");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;
    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn browser_decode_keeps_bgra_channels_and_transparency() {
        let png = vec![
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 1,
            8, 6, 0, 0, 0, 244, 34, 127, 138, 0, 0, 0, 14, 73, 68, 65, 84, 120, 156, 99, 248, 207,
            192, 0, 66, 13, 0, 15, 122, 3, 126, 119, 233, 127, 151, 0, 0, 0, 0, 73, 69, 78, 68,
            174, 66, 96, 130,
        ];
        let image = Arc::new(Image::from_bytes(gpui::ImageFormat::Png, png));
        let decoded = decode(Source::Bytes(image)).await.unwrap();
        assert_eq!(
            decoded.as_bytes(0).unwrap(),
            &[0, 0, 255, 255, 255, 0, 0, 128]
        );
    }

    #[wasm_bindgen_test]
    fn masks_keep_tint_alpha_and_reuse_the_cached_image() {
        let color: gpui::Hsla = gpui::Rgba {
            r: 1.,
            g: 0.,
            b: 0.,
            a: 0.5,
        }
        .into();
        let image = tinted_mask(&[128], 1, color);
        assert_eq!(image.as_bytes(0).unwrap(), &[0, 0, 255, 64]);
        assert!(Arc::ptr_eq(&image, &tinted_mask(&[128], 1, color)));
    }

    #[wasm_bindgen_test]
    async fn invalid_image_data_returns_an_error() {
        let image = Arc::new(Image::from_bytes(gpui::ImageFormat::Png, vec![0, 1, 2]));
        assert!(decode(Source::Bytes(image)).await.is_err());
    }
}
