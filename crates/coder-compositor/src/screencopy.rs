//! `wlr-screencopy`: the frame `grim`, `slurp`, and `wf-recorder` read.
//!
//! Smithay ships no screencopy, so the compositor answers
//! `zwlr_screencopy_manager_v1` itself. A client asks for one screen or a
//! rectangle of it, the compositor names the buffers it can fill, and the
//! client attaches one of that shape and asks for a copy. The copy happens
//! on the screen's next frame, from the same elements the frame draws, so
//! what the client reads is what the screen shows.
//!
//! The nested backend fills a shared-memory buffer out of the framebuffer
//! it just drew into its window. The hardware backend draws each copy into
//! a buffer of its own, because the DRM compositor scans its framebuffer
//! out and keeps it: into a texture it reads back for a shared-memory
//! buffer, or straight into the client's buffer when the client took a
//! dmabuf. The hardware backend offers a dmabuf for a copy of a whole
//! screen and a shared-memory buffer for every copy, so `grim -g` with a
//! rectangle takes shared memory on either backend.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::allocator::{Buffer, Fourcc};
use smithay::backend::renderer::element::RenderElement;
use smithay::backend::renderer::gles::GlesTexture;
use smithay::backend::renderer::utils::draw_render_elements;
use smithay::backend::renderer::{Bind, Color32F, ExportMem, Frame, Offscreen, Renderer};
use smithay::output::Output;
use smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::{
    zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
    zwlr_screencopy_manager_v1::{self, ZwlrScreencopyManagerV1},
};
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_shm;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};
use smithay::utils::{Rectangle, Transform};
use smithay::wayland::dmabuf::get_dmabuf;
use smithay::wayland::shm::with_buffer_contents;

use crate::layout::{self, Placed, Screen};
use crate::state::Coder;

/// The version of `zwlr_screencopy_manager_v1` the compositor answers.
/// Version 3 is what `grim` and `wf-recorder` bind.
pub const VERSION: u32 = 3;

/// The one shared-memory format the compositor writes. The renderer reads
/// back as four bytes of red, green, blue, and padding, which is this
/// format.
const FORMAT: wl_shm::Format = wl_shm::Format::Xbgr8888;

/// The shared-memory formats the compositor announces on `wl_shm`, on top
/// of the two every compositor announces.
///
/// A screencopy client allocates its buffer in the format the frame names,
/// so a compositor that names a format it does not accept on `wl_shm` is
/// answered with `format Xbgr8888 not supported` and captures nothing.
pub const SHM_FORMATS: [wl_shm::Format; 2] = [wl_shm::Format::Xbgr8888, wl_shm::Format::Abgr8888];

/// The format a dmabuf copy names. Every scanout format the hardware
/// backend picks from carries it.
pub const DMABUF_FORMAT: Fourcc = Fourcc::Xrgb8888;

/// The bytes one pixel of [`FORMAT`] takes.
const BYTES_PER_PIXEL: i32 = 4;

/// Which way a read-back's rows run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rows {
    /// The first row read is the bottom of the screen.
    ///
    /// `GlesRenderer::copy_framebuffer` reads with `glReadPixels`, whose
    /// origin is the lower left corner, and the nested backend renders so
    /// the image stands upright in a window's framebuffer of the same
    /// handedness. Its rows therefore arrive back to front, and the
    /// compositor turns them over rather than setting the protocol's
    /// `y_invert` flag, so a client reads the frame the right way up
    /// whether it honours the flag or not.
    BottomUp,
    /// The first row read is the top of the screen, which is what a frame
    /// drawn with no transform into a texture reads back as: the same
    /// orientation a dmabuf scanned out to a monitor has.
    TopDown,
}

/// The buffer a client attached.
#[derive(Debug)]
pub enum Target {
    /// Shared memory, which the compositor writes row by row.
    Shm,
    /// A dmabuf, which the compositor draws into.
    Dmabuf(Dmabuf),
}

/// What one client asked to copy, waiting for its screen's next frame.
pub struct Waiting {
    /// The frame the answer goes to.
    pub frame: ZwlrScreencopyFrameV1,
    /// The buffer the client attached.
    pub buffer: WlBuffer,
    /// The kind of buffer it is.
    pub target: Target,
    /// The screen the copy reads.
    pub output: String,
    /// The rectangle of the screen to copy, in pixels from the screen's top
    /// left corner.
    pub region: Placed,
    /// Whether the client asked for the damage the copy carries.
    pub with_damage: bool,
}

/// The screencopy globals and the copies waiting for a frame.
pub struct Screencopy {
    /// Holds the `zwlr_screencopy_manager_v1` global open for as long as
    /// the compositor runs.
    #[allow(dead_code)]
    global: smithay::reexports::wayland_server::backend::GlobalId,
    /// The copies the next frame answers.
    waiting: Vec<Waiting>,
    /// Whether a copy of a whole screen offers a dmabuf, which the hardware
    /// backend turns on once it has a renderer to draw into one.
    dmabuf: bool,
}

impl Screencopy {
    /// Announces the manager global.
    pub fn new(display: &DisplayHandle) -> Self {
        let global = display.create_global::<Coder, ZwlrScreencopyManagerV1, ()>(VERSION, ());
        Self {
            global,
            waiting: Vec::new(),
            dmabuf: false,
        }
    }

    /// Offers a dmabuf to every copy of a whole screen from now on.
    pub fn offer_dmabuf(&mut self) {
        self.dmabuf = true;
    }

    /// Takes the copies of one screen.
    pub fn take_for(&mut self, output: &str) -> Vec<Waiting> {
        let (taken, kept) = std::mem::take(&mut self.waiting)
            .into_iter()
            .partition(|copy| copy.output == output);
        self.waiting = kept;
        taken
    }
}

/// What one frame resource carries: the screen and rectangle it copies,
/// whether it offered a dmabuf, and whether the client has already used it.
/// The protocol allows one copy per frame.
pub struct FrameData {
    output: String,
    region: Placed,
    dmabuf: bool,
    used: AtomicBool,
}

/// The rectangle a request names, clamped to the screen.
///
/// A request for a rectangle that is partly off the screen is answered with
/// the part that is on it, and one that is wholly off the screen has no
/// pixels, which the copy reports as a failure.
pub fn clamp(screen: Screen, x: i32, y: i32, width: i32, height: i32) -> Placed {
    let whole = layout::whole(screen);
    let left = x.clamp(0, whole.width);
    let top = y.clamp(0, whole.height);
    let right = x.saturating_add(width.max(0)).clamp(left, whole.width);
    let bottom = y.saturating_add(height.max(0)).clamp(top, whole.height);
    Placed {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    }
}

/// A rectangle a client names in a screen's logical pixels, in that
/// screen's pixels. The edges round outward, so a copy of a window takes
/// every pixel the window touches.
pub fn to_pixels(x: i32, y: i32, width: i32, height: i32, scale: f64) -> (i32, i32, i32, i32) {
    let left = (f64::from(x) * scale).floor() as i32;
    let top = (f64::from(y) * scale).floor() as i32;
    let right = (f64::from(x.saturating_add(width.max(0))) * scale).ceil() as i32;
    let bottom = (f64::from(y.saturating_add(height.max(0))) * scale).ceil() as i32;
    (left, top, right - left, bottom - top)
}

/// The bytes one row of a copy of this rectangle takes.
pub fn stride(region: Placed) -> i32 {
    region.width.saturating_mul(BYTES_PER_PIXEL)
}

/// Where the renderer reads, in the framebuffer's own coordinates.
///
/// A read-back that runs from the bottom up counts a rectangle's top edge
/// as its distance from the bottom of the screen.
pub fn read_back_region(screen: Screen, region: Placed, rows: Rows) -> Placed {
    let y = match rows {
        Rows::BottomUp => (screen.height - region.y - region.height).max(0),
        Rows::TopDown => region.y,
    };
    Placed {
        x: region.x,
        y,
        width: region.width,
        height: region.height,
    }
}

/// Tells one client the shape of the buffers a copy can fill.
fn announce(frame: &ZwlrScreencopyFrameV1, region: Placed, dmabuf: bool) {
    frame.buffer(
        FORMAT,
        region.width.max(0) as u32,
        region.height.max(0) as u32,
        stride(region).max(0) as u32,
    );
    if frame.version() >= 3 {
        if dmabuf {
            frame.linux_dmabuf(
                DMABUF_FORMAT as u32,
                region.width.max(0) as u32,
                region.height.max(0) as u32,
            );
        }
        frame.buffer_done();
    }
}

/// The kind of buffer a client attached, when it can hold the copy.
fn buffer_fits(buffer: &WlBuffer, region: Placed, dmabuf: bool) -> Option<Target> {
    if let Ok(held) = get_dmabuf(buffer) {
        let size = held.size();
        let fits = dmabuf && size.w == region.width && size.h == region.height;
        return fits.then(|| Target::Dmabuf(held.clone()));
    }
    let read = with_buffer_contents(buffer, |_pointer, length, data| {
        let rows_fit = data.offset >= 0
            && data.stride >= stride(region)
            && i64::from(data.offset) + i64::from(data.stride) * i64::from(region.height.max(0))
                <= length as i64;
        let format_fits = data.format == FORMAT || data.format == wl_shm::Format::Abgr8888;
        format_fits && data.width == region.width && data.height == region.height && rows_fit
    });
    read.unwrap_or(false).then_some(Target::Shm)
}

impl GlobalDispatch<ZwlrScreencopyManagerV1, ()> for Coder {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<ZwlrScreencopyManagerV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for Coder {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &ZwlrScreencopyManagerV1,
        request: zwlr_screencopy_manager_v1::Request,
        _data: &(),
        _handle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        let (frame, output, rectangle) = match request {
            zwlr_screencopy_manager_v1::Request::CaptureOutput { frame, output, .. } => {
                (frame, output, None)
            }
            zwlr_screencopy_manager_v1::Request::CaptureOutputRegion {
                frame,
                output,
                x,
                y,
                width,
                height,
                ..
            } => (frame, output, Some((x, y, width, height))),
            zwlr_screencopy_manager_v1::Request::Destroy => return,
            _ => return,
        };
        let screen = Output::from_resource(&output).and_then(|output| {
            let mode = output.current_mode()?;
            let scale = output.current_scale().fractional_scale();
            Some((
                output.name(),
                Screen {
                    width: mode.size.w,
                    height: mode.size.h,
                },
                scale,
            ))
        });
        let Some((name, mode, scale)) = screen else {
            let frame = data_init.init(
                frame,
                FrameData {
                    output: String::new(),
                    region: Placed {
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                    },
                    dmabuf: false,
                    used: AtomicBool::new(false),
                },
            );
            frame.failed();
            return;
        };
        let region = match rectangle {
            None => layout::whole(mode),
            Some((x, y, width, height)) => {
                let (x, y, width, height) = to_pixels(x, y, width, height, scale);
                clamp(mode, x, y, width, height)
            }
        };
        let dmabuf = state.screencopy.dmabuf && rectangle.is_none();
        let frame = data_init.init(
            frame,
            FrameData {
                output: name,
                region,
                dmabuf,
                used: AtomicBool::new(false),
            },
        );
        announce(&frame, region, dmabuf);
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, FrameData> for Coder {
    fn request(
        state: &mut Self,
        _client: &Client,
        resource: &ZwlrScreencopyFrameV1,
        request: zwlr_screencopy_frame_v1::Request,
        data: &FrameData,
        _handle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        let (buffer, with_damage) = match request {
            zwlr_screencopy_frame_v1::Request::Copy { buffer } => (buffer, false),
            zwlr_screencopy_frame_v1::Request::CopyWithDamage { buffer } => (buffer, true),
            zwlr_screencopy_frame_v1::Request::Destroy => {
                state
                    .screencopy
                    .waiting
                    .retain(|held| &held.frame != resource);
                return;
            }
            _ => return,
        };
        if data.used.swap(true, Ordering::SeqCst) {
            resource.post_error(
                zwlr_screencopy_frame_v1::Error::AlreadyUsed,
                "a frame copies once",
            );
            return;
        }
        if data.region.width <= 0 || data.region.height <= 0 {
            resource.failed();
            return;
        }
        let Some(target) = buffer_fits(&buffer, data.region, data.dmabuf) else {
            resource.post_error(
                zwlr_screencopy_frame_v1::Error::InvalidBuffer,
                "the buffer does not match the frame the compositor named",
            );
            return;
        };
        state.screencopy.waiting.push(Waiting {
            frame: resource.clone(),
            buffer,
            target,
            output: data.output.clone(),
            region: data.region,
            with_damage,
        });
    }

    fn destroyed(
        state: &mut Self,
        _client: ClientId,
        resource: &ZwlrScreencopyFrameV1,
        _data: &FrameData,
    ) {
        state
            .screencopy
            .waiting
            .retain(|held| &held.frame != resource);
    }
}

/// One copy's pixels, read out of the framebuffer and not yet handed over.
pub struct Captured<M> {
    copy: Waiting,
    read: Result<M, String>,
}

/// Reads every waiting shared-memory copy out of the framebuffer the nested
/// backend just drew.
///
/// The framebuffer holds what the compositor just drew and the screen has
/// not shown it yet, so the pixels a client reads are the pixels the next
/// refresh puts on the screen. The read leaves the pixels on the graphics
/// card; [`deliver`] brings them across. A dmabuf copy is refused here,
/// because the nested backend never offers one.
pub fn capture<R: ExportMem>(
    waiting: Vec<Waiting>,
    renderer: &mut R,
    framebuffer: &R::Framebuffer<'_>,
    screen: Screen,
) -> Vec<Captured<R::TextureMapping>> {
    waiting
        .into_iter()
        .map(|copy| {
            let read = match copy.target {
                Target::Shm => {
                    read_back(renderer, framebuffer, screen, copy.region, Rows::BottomUp)
                }
                Target::Dmabuf(_) => Err("the nested screen copies into shared memory".to_string()),
            };
            Captured { copy, read }
        })
        .collect()
}

fn read_back<R: ExportMem>(
    renderer: &mut R,
    framebuffer: &R::Framebuffer<'_>,
    screen: Screen,
    region: Placed,
    rows: Rows,
) -> Result<R::TextureMapping, String> {
    let region = read_back_region(screen, region, rows);
    let rectangle = Rectangle::new(
        (region.x, region.y).into(),
        (region.width, region.height).into(),
    );
    renderer
        .copy_framebuffer(framebuffer, rectangle, Fourcc::Xbgr8888)
        .map_err(|err| format!("the renderer did not read the screen back: {err}"))
}

/// Writes every read into the buffer its client attached and answers the
/// frame.
///
/// The nested backend runs this after the frame reaches the screen, because
/// mapping a read makes the renderer's own context current and the screen's
/// swap needs the window's.
pub fn deliver<R: ExportMem>(
    captured: Vec<Captured<R::TextureMapping>>,
    renderer: &mut R,
    time: Duration,
    rows: Rows,
) {
    for Captured { copy, read } in captured {
        let written = read.and_then(|mapping| {
            let pixels = renderer
                .map_texture(&mapping)
                .map_err(|err| format!("the read-back did not map: {err}"))?;
            write_rows(&copy, pixels, rows)
        });
        answer(copy, written, time);
    }
}

/// Reads a whole screen out of the framebuffer the nested backend just
/// drew, for a `shot` the desk protocol asked for. The read is brought
/// across with [`mapped`] after the frame reaches the screen, for the same
/// reason a client's copy is.
pub fn read_screen<R: ExportMem>(
    renderer: &mut R,
    framebuffer: &R::Framebuffer<'_>,
    screen: Screen,
) -> Result<R::TextureMapping, String> {
    read_back(renderer, framebuffer, screen, whole(screen), Rows::BottomUp)
}

/// Brings one read across, as the bytes a `shot` writes.
pub fn mapped<R: ExportMem>(
    renderer: &mut R,
    mapping: &R::TextureMapping,
) -> Result<Vec<u8>, String> {
    renderer
        .map_texture(mapping)
        .map(<[u8]>::to_vec)
        .map_err(|err| format!("the read-back did not map: {err}"))
}

/// Draws one screen into a buffer of its own and answers its pixels, which
/// is how the hardware backend fills a `shot`: the DRM compositor scans its
/// framebuffer out and keeps it.
pub fn render_screen<R, E>(
    renderer: &mut R,
    elements: &[E],
    mode: Screen,
    scale: f64,
) -> Result<Vec<u8>, String>
where
    R: Renderer + Offscreen<GlesTexture> + ExportMem,
    R::TextureId: 'static,
    E: RenderElement<R>,
{
    let mut texture = renderer
        .create_buffer(Fourcc::Xbgr8888, (mode.width, mode.height).into())
        .map_err(|err| format!("the shot's texture was not made: {err}"))?;
    let mut framebuffer = renderer
        .bind(&mut texture)
        .map_err(|err| format!("the shot's texture did not bind: {err}"))?;
    draw_frame(renderer, &mut framebuffer, elements, mode, scale)?;
    let mapping = read_back(renderer, &framebuffer, mode, whole(mode), Rows::TopDown)?;
    mapped(renderer, &mapping)
}

/// The rectangle one whole screen covers.
fn whole(screen: Screen) -> Placed {
    Placed {
        x: 0,
        y: 0,
        width: screen.width,
        height: screen.height,
    }
}

/// Draws every waiting copy of one screen into a buffer of its own and
/// answers it, which is how the hardware backend copies: the DRM compositor
/// keeps the framebuffer it scans out.
///
/// `elements` is the list the screen's frame drew, `mode` the screen's size
/// in pixels, and `scale` its scale.
pub fn draw_copies<R, E>(
    waiting: Vec<Waiting>,
    renderer: &mut R,
    elements: &[E],
    mode: Screen,
    scale: f64,
    time: Duration,
) where
    R: Renderer + Offscreen<GlesTexture> + Bind<Dmabuf> + ExportMem,
    R::TextureId: 'static,
    E: RenderElement<R>,
{
    for copy in waiting {
        // A dmabuf is a handle on shared buffers, so the copy binds a handle
        // of its own and the waiting copy stays readable for the answer.
        let written = match &copy.target {
            Target::Shm => draw_to_memory(renderer, elements, mode, scale, &copy),
            Target::Dmabuf(dmabuf) => {
                draw_to_dmabuf(renderer, elements, mode, scale, &mut dmabuf.clone())
            }
        };
        answer(copy, written, time);
    }
}

fn draw_to_memory<R, E>(
    renderer: &mut R,
    elements: &[E],
    mode: Screen,
    scale: f64,
    copy: &Waiting,
) -> Result<(), String>
where
    R: Renderer + Offscreen<GlesTexture> + ExportMem,
    R::TextureId: 'static,
    E: RenderElement<R>,
{
    let mut texture = renderer
        .create_buffer(Fourcc::Xbgr8888, (mode.width, mode.height).into())
        .map_err(|err| format!("the copy's texture was not made: {err}"))?;
    let mut framebuffer = renderer
        .bind(&mut texture)
        .map_err(|err| format!("the copy's texture did not bind: {err}"))?;
    draw_frame(renderer, &mut framebuffer, elements, mode, scale)?;
    let mapping = read_back(renderer, &framebuffer, mode, copy.region, Rows::TopDown)?;
    let pixels = renderer
        .map_texture(&mapping)
        .map_err(|err| format!("the read-back did not map: {err}"))?;
    write_rows(copy, pixels, Rows::TopDown)
}

fn draw_to_dmabuf<R, E>(
    renderer: &mut R,
    elements: &[E],
    mode: Screen,
    scale: f64,
    dmabuf: &mut Dmabuf,
) -> Result<(), String>
where
    R: Renderer + Bind<Dmabuf>,
    R::TextureId: 'static,
    E: RenderElement<R>,
{
    let mut framebuffer = renderer
        .bind(dmabuf)
        .map_err(|err| format!("the client's dmabuf did not bind: {err}"))?;
    draw_frame(renderer, &mut framebuffer, elements, mode, scale)
}

/// Draws one frame of a screen into a bound buffer and waits for the
/// graphics card to finish it, since the client reads the buffer as soon as
/// the frame is answered.
fn draw_frame<R, E>(
    renderer: &mut R,
    framebuffer: &mut R::Framebuffer<'_>,
    elements: &[E],
    mode: Screen,
    scale: f64,
) -> Result<(), String>
where
    R: Renderer,
    R::TextureId: 'static,
    E: RenderElement<R>,
{
    let size = (mode.width, mode.height).into();
    let whole = Rectangle::from_size(size);
    let mut frame = renderer
        .render(framebuffer, size, Transform::Normal)
        .map_err(|err| format!("the copy's frame did not start: {err}"))?;
    frame
        .clear(Color32F::from(layout::BACKGROUND), &[whole])
        .map_err(|err| format!("the copy's frame did not clear: {err}"))?;
    draw_render_elements(&mut frame, scale, elements, &[whole])
        .map_err(|err| format!("the copy's frame did not draw: {err}"))?;
    let sync = frame
        .finish()
        .map_err(|err| format!("the copy's frame did not finish: {err}"))?;
    sync.wait()
        .map_err(|_| "the copy's frame was interrupted".to_string())
}

/// Answers one frame with the outcome of its copy.
fn answer(copy: Waiting, written: Result<(), String>, time: Duration) {
    match written {
        Ok(()) => {
            copy.frame.flags(zwlr_screencopy_frame_v1::Flags::empty());
            if copy.with_damage && copy.frame.version() >= 2 {
                copy.frame.damage(
                    0,
                    0,
                    copy.region.width.max(0) as u32,
                    copy.region.height.max(0) as u32,
                );
            }
            let seconds = time.as_secs();
            copy.frame.ready(
                (seconds >> 32) as u32,
                (seconds & 0xffff_ffff) as u32,
                time.subsec_nanos(),
            );
        }
        Err(err) => {
            log::warn!("a screen copy did not finish: {err}");
            copy.frame.failed();
        }
    }
}

/// Copies a read-back into the client's buffer, one row at a time.
fn write_rows(copy: &Waiting, pixels: &[u8], rows: Rows) -> Result<(), String> {
    let width = copy.region.width.max(0) as usize;
    let height = copy.region.height.max(0) as usize;
    let row = width * BYTES_PER_PIXEL as usize;
    if pixels.len() < row * height {
        return Err(format!(
            "the read-back holds {} bytes and the frame needs {}",
            pixels.len(),
            row * height
        ));
    }
    let written =
        smithay::wayland::shm::with_buffer_contents_mut(&copy.buffer, |pointer, length, data| {
            if data.offset < 0 || data.stride < row as i32 {
                return Err("the buffer's rows are shorter than the frame".to_string());
            }
            // SAFETY: the pointer and the length come from the pool the
            // client shared with the compositor, and Smithay holds the
            // mapping for as long as this closure runs. Every write below
            // goes through the slice the two describe, and each row's start
            // and end are checked against its length first.
            let pool = unsafe { std::slice::from_raw_parts_mut(pointer, length) };
            for line in 0..height {
                let source = match rows {
                    Rows::BottomUp => height - 1 - line,
                    Rows::TopDown => line,
                };
                let from = source * row;
                let at = data.offset as usize + line * data.stride as usize;
                let Some(target) = pool.get_mut(at..at + row) else {
                    return Err("the buffer is shorter than the frame".to_string());
                };
                target.copy_from_slice(&pixels[from..from + row]);
            }
            Ok(())
        });
    match written {
        Ok(result) => result,
        Err(err) => Err(format!("the client's buffer did not map: {err}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Screen = Screen {
        width: 1280,
        height: 800,
    };

    #[test]
    fn a_capture_of_the_whole_screen_is_the_whole_screen() {
        let region = clamp(SCREEN, 0, 0, SCREEN.width, SCREEN.height);
        assert_eq!(region, layout::whole(SCREEN));
        assert_eq!(stride(region), SCREEN.width * 4);
    }

    #[test]
    fn a_rectangle_that_hangs_off_the_screen_is_cut_to_it() {
        let region = clamp(SCREEN, 1200, 700, 400, 400);
        assert_eq!(region.x, 1200);
        assert_eq!(region.y, 700);
        assert_eq!(region.width, 80);
        assert_eq!(region.height, 100);
    }

    #[test]
    fn a_rectangle_wholly_off_the_screen_has_no_pixels() {
        let region = clamp(SCREEN, 4000, 4000, 100, 100);
        assert_eq!(region.width, 0);
        assert_eq!(region.height, 0);
    }

    #[test]
    fn a_negative_origin_starts_at_the_screen_edge() {
        let region = clamp(SCREEN, -50, -20, 200, 100);
        assert_eq!(region.x, 0);
        assert_eq!(region.y, 0);
        assert_eq!(region.width, 150);
        assert_eq!(region.height, 80);
    }

    #[test]
    fn the_read_back_counts_rows_from_the_bottom_of_the_screen() {
        let region = Placed {
            x: 10,
            y: 0,
            width: 100,
            height: 50,
        };
        let read = read_back_region(SCREEN, region, Rows::BottomUp);
        assert_eq!(read.x, 10);
        assert_eq!(read.y, SCREEN.height - 50);
        assert_eq!(read.width, 100);
        assert_eq!(read.height, 50);
    }

    #[test]
    fn a_rectangle_at_the_bottom_of_the_screen_reads_at_the_origin() {
        let region = Placed {
            x: 0,
            y: SCREEN.height - 50,
            width: 100,
            height: 50,
        };
        let read = read_back_region(SCREEN, region, Rows::BottomUp);
        assert_eq!(read.y, 0);
    }

    #[test]
    fn the_read_back_of_the_whole_screen_starts_at_the_origin() {
        let read = read_back_region(SCREEN, layout::whole(SCREEN), Rows::BottomUp);
        assert_eq!(read.x, 0);
        assert_eq!(read.y, 0);
    }

    #[test]
    fn a_texture_the_hardware_backend_drew_reads_from_the_top() {
        let region = Placed {
            x: 10,
            y: 30,
            width: 100,
            height: 50,
        };
        assert_eq!(read_back_region(SCREEN, region, Rows::TopDown), region);
    }

    #[test]
    fn a_rectangle_on_a_scaled_screen_takes_every_pixel_it_touches() {
        assert_eq!(to_pixels(0, 0, 100, 60, 1.0), (0, 0, 100, 60));
        assert_eq!(to_pixels(0, 0, 100, 60, 1.25), (0, 0, 125, 75));
        assert_eq!(to_pixels(1, 1, 1, 1, 1.25), (1, 1, 2, 2));
    }
}
