//! Bounded decoded stereo streaming. Feed and decode outside the mixer callback.
use rtrb::{Consumer, Producer, PushError, RingBuffer};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub type Frame = [f32; 2];
pub struct Feeder {
    producer: Producer<Frame>,
    finished: Arc<AtomicBool>,
}
pub struct Stream {
    pub(crate) rate: u32,
    consumer: Consumer<Frame>,
    finished: Arc<AtomicBool>,
    current: Option<Frame>,
    next: Option<Frame>,
    phase: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedError {
    Invalid,
    Full,
    Closed,
}
/// The queue holds decoded frames only; no file, decoder, or lock reaches render.
pub fn channel(rate: u32, frames: usize) -> Result<(Feeder, Stream), FeedError> {
    if !(8000..=192000).contains(&rate) || !(128..=65536).contains(&frames) {
        return Err(FeedError::Invalid);
    }
    let (producer, consumer) = RingBuffer::new(frames);
    let finished = Arc::new(AtomicBool::new(false));
    Ok((
        Feeder {
            producer,
            finished: finished.clone(),
        },
        Stream {
            rate,
            consumer,
            finished,
            current: None,
            next: None,
            phase: 0.0,
        },
    ))
}
impl Feeder {
    pub fn slots(&self) -> usize {
        self.producer.slots()
    }
    pub fn closed(&self) -> bool {
        self.producer.is_abandoned()
    }
    pub fn push(&mut self, frame: Frame) -> Result<(), FeedError> {
        if frame.iter().any(|s| !s.is_finite() || s.abs() > 1.0) {
            return Err(FeedError::Invalid);
        }
        if self.closed() || self.finished.load(Ordering::Acquire) {
            return Err(FeedError::Closed);
        }
        self.producer
            .push(frame)
            .map_err(|PushError::Full(_)| FeedError::Full)
    }
    pub fn finish(&mut self) {
        self.finished.store(true, Ordering::Release);
    }
}
impl Stream {
    fn finished(&self) -> bool {
        self.finished.load(Ordering::Acquire) || self.consumer.is_abandoned()
    }
    /// None is a temporary underrun; an empty finished source is End.
    pub(crate) fn sample(&mut self, step: f64) -> Sample {
        let available = self.consumer.slots()
            + usize::from(self.current.is_some())
            + usize::from(self.next.is_some());
        if !self.finished() && available < step.ceil() as usize + 2 {
            return Sample::Gap;
        }
        if self.current.is_none() {
            self.current = self.consumer.pop().ok();
        }
        if self.next.is_none() {
            self.next = self.consumer.pop().ok();
        }
        let Some(current) = self.current else {
            return if self.finished() {
                Sample::End
            } else {
                Sample::Gap
            };
        };
        let next = self.next.unwrap_or([0.0; 2]);
        let out = [0, 1].map(|i| current[i] + (next[i] - current[i]) * self.phase as f32);
        self.phase += step;
        while self.phase >= 1.0 {
            self.phase -= 1.0;
            self.current = self.next.take();
            self.next = self.consumer.pop().ok();
        }
        Sample::Frame(out)
    }
}
pub(crate) enum Sample {
    Frame(Frame),
    Gap,
    End,
}

/// Reads canonical little-endian stereo float PCM in bounded batches. The caller
/// supplies an admitted reader; this module chooses no path and opens no file.
pub struct Reader<R> {
    reader: R,
    feeder: Feeder,
    remaining: u64,
}
impl<R: std::io::Read> Reader<R> {
    pub fn new(reader: R, feeder: Feeder, frames: u64) -> Result<Self, FeedError> {
        if frames == 0 || frames > 192000 * 86400 {
            return Err(FeedError::Invalid);
        }
        Ok(Self {
            reader,
            feeder,
            remaining: frames,
        })
    }
    /// At most 1024 frames per call, with validation before each publication.
    /// A decode error terminates the source; already admitted frames may drain.
    pub fn pump(&mut self) -> Result<usize, String> {
        let count = self.remaining.min(self.feeder.slots() as u64).min(1024) as usize;
        let mut bytes = [0u8; 8192];
        if let Err(error) = self.reader.read_exact(&mut bytes[..count * 8]) {
            self.feeder.finish();
            return Err(error.to_string());
        }
        let mut frames = [[0.0; 2]; 1024];
        for (frame, bytes) in frames[..count].iter_mut().zip(bytes.chunks_exact(8)) {
            *frame = [
                f32::from_le_bytes(bytes[..4].try_into().unwrap()),
                f32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            ];
            if frame.iter().any(|s| !s.is_finite() || s.abs() > 1.0) {
                self.feeder.finish();
                return Err("Invalid decoded audio frame".into());
            }
        }
        for frame in frames[..count].iter().copied() {
            self.feeder
                .push(frame)
                .map_err(|_| "Audio streaming consumer closed")?;
        }
        self.remaining -= count as u64;
        if self.remaining == 0 {
            self.feeder.finish();
        }
        Ok(count)
    }
    pub fn remaining(&self) -> u64 {
        self.remaining
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fragmented_pcm_backpressure_and_end_are_bounded() {
        struct Fragment(std::io::Cursor<Vec<u8>>);
        impl std::io::Read for Fragment {
            fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
                let len = b.len().min(3);
                std::io::Read::read(&mut self.0, &mut b[..len])
            }
        }
        let (feeder, mut stream) = channel(48000, 128).unwrap();
        let bytes = (0..256)
            .flat_map(|_| [0.25f32, -0.5].into_iter().flat_map(f32::to_le_bytes))
            .collect();
        let mut reader = Reader::new(Fragment(std::io::Cursor::new(bytes)), feeder, 256).unwrap();
        assert_eq!(reader.pump().unwrap(), 128);
        assert_eq!(reader.pump().unwrap(), 0);
        for _ in 0..126 {
            assert!(matches!(stream.sample(1.), Sample::Frame([0.25, -0.5])));
        }
        assert!(matches!(stream.sample(1.), Sample::Gap));
        assert!(reader.pump().unwrap() > 0);
        while reader.remaining() > 0 {
            for _ in 0..64 {
                let _ = stream.sample(1.);
            }
            reader.pump().unwrap();
        }
        let mut ended = false;
        for _ in 0..256 {
            if matches!(stream.sample(1.), Sample::End) {
                ended = true;
                break;
            }
        }
        assert!(ended);
    }
    #[test]
    fn invalid_batch_publishes_no_frames_and_abandonment_ends_source() {
        let (feeder, mut stream) = channel(48000, 128).unwrap();
        let bytes = [0.2f32, 0.2, f32::NAN, 0.]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        let mut reader = Reader::new(std::io::Cursor::new(bytes), feeder, 2).unwrap();
        assert!(reader.pump().is_err());
        assert!(matches!(stream.sample(1.), Sample::End));
        let (feeder, mut stream) = channel(48000, 128).unwrap();
        drop(feeder);
        assert!(matches!(stream.sample(1.), Sample::End));
    }
}
