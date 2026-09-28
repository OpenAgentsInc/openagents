//! The camera, read with V4L2 and decoded once.
//!
//! The daemon asks the node for MJPEG at the grant's size and rate, which
//! is what a USB camera delivers at 720p30 over one bus, and falls back
//! to YUYV. Each frame is decoded to RGB here, once, and every output
//! reads that one decode.

/// The errno the kernel answers when another process streams the node.
pub const EBUSY: i32 = 16;

/// One camera error, with the device, the stage, and the kernel's words.
/// `EBUSY` says who it is on CoderOS: the node streams to one process,
/// and the other process is the camera circle's player on a host whose
/// session started the circle on the camera itself, or a second daemon.
pub fn describe(device: &str, stage: &str, err: &std::io::Error) -> String {
    if err.raw_os_error() == Some(EBUSY) {
        format!(
            "another process streams {device} (the camera circle on the camera itself, or a second daemon); this daemon has to be the one that opens it"
        )
    } else {
        format!("{device}{stage}: {err}")
    }
}

#[cfg(target_os = "linux")]
pub use linux::Capture;

#[cfg(target_os = "linux")]
mod linux {
    use super::describe;
    use crate::frame::{Frame, mjpeg_to_rgb, now, yuyv_to_rgb};
    use v4l::FourCC;
    use v4l::buffer::Type;
    use v4l::io::traits::CaptureStream;
    use v4l::prelude::*;
    use v4l::video::Capture as _;
    use v4l::video::capture::Parameters;

    /// An open camera streaming one format.
    pub struct Capture {
        stream: MmapStream<'static>,
        pub width: u32,
        pub height: u32,
        pub fourcc: String,
        yuyv: bool,
        seq: u64,
    }

    impl Capture {
        /// Opens `device` at `width` by `height` and `fps`, MJPEG first.
        pub fn open(device: &str, width: u32, height: u32, fps: u32) -> Result<Capture, String> {
            let dev = Device::with_path(device).map_err(|err| describe(device, "", &err))?;
            let mut fmt = dev
                .format()
                .map_err(|err| describe(device, " format", &err))?;
            fmt.width = width;
            fmt.height = height;
            fmt.fourcc = FourCC::new(b"MJPG");
            let fmt = match dev.set_format(&fmt) {
                Ok(f) if f.fourcc == FourCC::new(b"MJPG") => f,
                _ => {
                    fmt.fourcc = FourCC::new(b"YUYV");
                    dev.set_format(&fmt)
                        .map_err(|err| describe(device, " format", &err))?
                }
            };
            if let Err(err) = dev.set_params(&Parameters::with_fps(fps)) {
                eprintln!("coderos-camera: {device} keeps its own frame rate: {err}");
            }
            let yuyv = fmt.fourcc == FourCC::new(b"YUYV");
            let fourcc = fmt.fourcc.to_string();
            // The stream borrows the device for as long as it runs, which is
            // the life of the daemon.
            let dev = Box::leak(Box::new(dev));
            let stream = MmapStream::with_buffers(dev, Type::VideoCapture, 4)
                .map_err(|err| describe(device, " stream", &err))?;
            Ok(Capture {
                stream,
                width: fmt.width,
                height: fmt.height,
                fourcc,
                yuyv,
                seq: 0,
            })
        }

        /// The next frame, decoded.
        pub fn next(&mut self) -> Result<Frame, String> {
            let (buf, meta) = self.stream.next().map_err(|err| format!("frame: {err}"))?;
            let used = (meta.bytesused as usize).min(buf.len());
            let buf = if used > 0 { &buf[..used] } else { buf };
            let timestamp = now();
            let (rgb, width, height) = if self.yuyv {
                (
                    yuyv_to_rgb(buf, self.width, self.height),
                    self.width,
                    self.height,
                )
            } else {
                mjpeg_to_rgb(buf)?
            };
            self.seq += 1;
            Ok(Frame {
                seq: self.seq,
                timestamp,
                width,
                height,
                rgb,
            })
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub use other::Capture;

#[cfg(not(target_os = "linux"))]
mod other {
    use super::describe;
    use crate::frame::Frame;

    /// A camera on a platform with no V4L2, which opens nothing.
    pub struct Capture {
        pub width: u32,
        pub height: u32,
        pub fourcc: String,
    }

    impl Capture {
        pub fn open(device: &str, _width: u32, _height: u32, _fps: u32) -> Result<Capture, String> {
            let err = std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "this platform has no V4L2; the daemon runs on a CoderOS host",
            );
            Err(describe(device, "", &err))
        }

        pub fn next(&mut self) -> Result<Frame, String> {
            Err("no camera on this platform".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_busy_node_names_who_holds_it_and_another_error_keeps_the_stage() {
        let busy = describe(
            "/dev/video0",
            " format",
            &std::io::Error::from_raw_os_error(EBUSY),
        );
        assert!(
            busy.starts_with("another process streams /dev/video0"),
            "{busy}"
        );
        let gone = describe(
            "/dev/video0",
            " stream",
            &std::io::Error::from_raw_os_error(2),
        );
        assert!(gone.starts_with("/dev/video0 stream: "), "{gone}");
    }
}
