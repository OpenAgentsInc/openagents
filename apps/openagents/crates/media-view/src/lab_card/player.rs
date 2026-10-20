//! Inline video for Lab cards on macOS: AVFoundation decodes (with sound,
//! seeking, and HTTP range reads), and each new frame comes back as a
//! CoreVideo pixel buffer that GPUI draws with `surface` like any element, so
//! the video scrolls, clips and fades with the transcript. No native view is
//! hosted.

use core_foundation::base::TCFType as _;
use objc2::AnyThread as _;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_av_foundation::{AVPlayer, AVPlayerItem, AVPlayerItemVideoOutput};
use objc2_core_media::CMTime;
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};

/// GPUI's surface draws bi-planar full-range 4:2:0 ('420f').
const PIXEL_FORMAT_420F: u32 = 0x3432_3066;

pub struct VideoPlayer {
    player: Retained<AVPlayer>,
    item: Retained<AVPlayerItem>,
    output: Retained<AVPlayerItemVideoOutput>,
    frame: Option<core_video::pixel_buffer::CVPixelBuffer>,
}

impl VideoPlayer {
    /// A paused player for `url`. Main thread only (AVPlayer's own rule).
    pub fn new(url: &str) -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        unsafe {
            let url = NSURL::URLWithString(&NSString::from_str(url))?;
            let item = AVPlayerItem::playerItemWithURL(&url, mtm);
            let format = NSNumber::new_u32(PIXEL_FORMAT_420F);
            let metal = NSNumber::new_bool(true);
            let keys = [
                NSString::from_str("PixelFormatType"),
                NSString::from_str("MetalCompatibility"),
            ];
            let values: [&AnyObject; 2] = [&format, &metal];
            let attributes = NSDictionary::from_slices(&[&*keys[0], &*keys[1]], &values);
            let output = AVPlayerItemVideoOutput::initWithPixelBufferAttributes(
                AVPlayerItemVideoOutput::alloc(),
                Some(&attributes),
            );
            item.addOutput(&output);
            let player = AVPlayer::playerWithPlayerItem(Some(&item), mtm);
            Some(Self {
                player,
                item,
                output,
                frame: None,
            })
        }
    }

    pub fn play(&self) {
        unsafe {
            let (now, end) = (self.position(), self.duration());
            if end > 0.0 && now >= end - 0.05 {
                self.seek(0.0);
            }
            self.player.play();
        }
    }

    pub fn pause(&self) {
        unsafe { self.player.pause() }
    }

    pub fn playing(&self) -> bool {
        unsafe { self.player.rate() > 0.0 }
    }

    pub fn seek(&self, seconds: f64) {
        unsafe {
            let zero = CMTime::with_seconds(0.0, 600);
            self.player.seekToTime_toleranceBefore_toleranceAfter(
                CMTime::with_seconds(seconds.max(0.0), 600),
                zero,
                zero,
            );
        }
    }

    pub fn position(&self) -> f64 {
        finite(unsafe { self.item.currentTime().seconds() })
    }

    pub fn duration(&self) -> f64 {
        finite(unsafe { self.item.duration().seconds() })
    }

    /// The latest decoded frame, pulled when a newer one is ready.
    pub fn frame(&mut self) -> Option<core_video::pixel_buffer::CVPixelBuffer> {
        unsafe {
            let now = self.item.currentTime();
            if self.output.hasNewPixelBufferForItemTime(now)
                && let Some(buffer) = self
                    .output
                    .copyPixelBufferForItemTime_itemTimeForDisplay(now, std::ptr::null_mut())
            {
                let raw = Retained::as_ptr(&buffer) as *const _
                    as core_video::pixel_buffer::CVPixelBufferRef;
                self.frame =
                    Some(core_video::pixel_buffer::CVPixelBuffer::wrap_under_get_rule(raw));
            }
        }
        self.frame.clone()
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        self.pause();
    }
}

fn finite(seconds: f64) -> f64 {
    if seconds.is_finite() {
        seconds.max(0.0)
    } else {
        0.0
    }
}
