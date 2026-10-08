//! Water pass timestamps with three bounded delayed readback slots.
//! Unsupported APIs report `None`; a queue fence is not a GPU timestamp.
use std::sync::mpsc::{self, Receiver};

/// CPU time of the current thread, when the platform supplies a clock.
/// Browsers expose elapsed time only, so they keep this measurement absent.
pub(crate) fn cpu_ms() -> Option<f64> {
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "linux",
        target_os = "android"
    ))]
    {
        let mut time = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: clock_gettime writes one initialized, writable timespec.
        if unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) } == 0 {
            return Some(time.tv_sec as f64 * 1000.0 + time.tv_nsec as f64 / 1e6);
        }
    }
    None
}

pub(crate) struct CpuTimer {
    wall: web_time::Instant,
    cpu: Option<f64>,
}
impl CpuTimer {
    pub fn start() -> Self {
        Self {
            wall: web_time::Instant::now(),
            cpu: cpu_ms(),
        }
    }
    pub fn elapsed_ms(&self) -> f64 {
        self.wall.elapsed().as_secs_f64() * 1000.0
    }
    pub fn cpu_ms(&self) -> Option<f64> {
        Some((cpu_ms()? - self.cpu?).max(0.0))
    }
}

/// A completed fixed-view sample: mirror, opaque scene, copies, surface.
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct GpuSample {
    pub frame: u64,
    pub pass_ms: [Option<f64>; 4],
    pub water_ms: f64,
}

/// Water's measured work and owned resources for the last displayed frame.
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct Measurements {
    pub gpu: Option<GpuSample>,
    pub gpu_timestamps: bool,
    pub gpu_bytes: u64,
    /// Elapsed preparation, mirror, copy, and surface encoding, excluding GPU waits.
    pub main_ms: f64,
    pub main_cpu_ms: Option<f64>,
    /// Last job duration, reported separately from completed interval work.
    pub synthesis_ms: f64,
    pub synthesis_cpu_ms: Option<f64>,
    pub completed_jobs: u64,
    pub completed_synthesis_ms: f64,
    /// CPU work of jobs received this display frame; zero for inline waves.
    pub worker_ms: f64,
    pub worker_cpu_supported: bool,
    pub worker_bytes: u64,
    pub ripple_cpu_bytes: u64,
    pub inline_synthesis: bool,
    pub refresh_every: u32,
    pub copies: bool,
    pub mirror: bool,
    pub ssr: bool,
    pub effects_reduced: bool,
}

struct Slot {
    query: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    pending: Option<(u64, u8, Receiver<Result<(), wgpu::BufferAsyncError>>)>,
    awaiting: Option<(u64, u8, Receiver<()>)>,
}

pub(crate) struct Timer {
    slots: Vec<Slot>,
    frame: u64,
    period: f64,
    queue: wgpu::Queue,
}

/// Charge dependent passes by their completion frontier on tile GPUs,
/// without counting overlapping time twice or retaining stale pass values.
fn sample(frame: u64, ticks: [u64; 8], mask: u8, period: f64) -> Option<GpuSample> {
    if !period.is_finite() || period <= 0.0 || mask & 8 == 0 {
        return None;
    }
    let mut order = Vec::new();
    for i in 0..4 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let (start, end) = (ticks[2 * i], ticks[2 * i + 1]);
        if end <= start {
            return None;
        }
        order.push((end, start, i));
    }
    order.sort_unstable();
    let mut result = GpuSample {
        frame,
        ..GpuSample::default()
    };
    let mut previous = 0;
    for (end, start, i) in order {
        // Copy and surface time starts at the previous completion frontier.
        // This charges the color-copy command between opaque and depth-copy
        // passes, which cannot carry a render-pass timestamp itself.
        let begin = if i >= 2 && previous > 0 {
            previous
        } else {
            start.max(previous)
        };
        let ms = end.saturating_sub(begin) as f64 * period / 1e6;
        if !ms.is_finite() || ms > 1000.0 {
            return None;
        }
        result.pass_ms[i] = Some(ms);
        if i != 1 {
            result.water_ms += ms;
        }
        previous = end;
    }
    Some(result)
}

impl Timer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        Some(Self {
            queue: queue.clone(),
            frame: 0,
            period: f64::from(queue.get_timestamp_period()),
            slots: (0..3)
                .map(|_| Slot {
                    query: device.create_query_set(&wgpu::QuerySetDescriptor {
                        label: Some("Verse water pass timestamps"),
                        ty: wgpu::QueryType::Timestamp,
                        count: 8,
                    }),
                    resolve: device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("Verse water query resolve"),
                        size: 64,
                        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                        mapped_at_creation: false,
                    }),
                    readback: device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("Verse water delayed timestamps"),
                        size: 64,
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    }),
                    pending: None,
                    awaiting: None,
                })
                .collect(),
        })
    }
    pub fn begin(&mut self, device: &wgpu::Device) -> (Option<usize>, Option<GpuSample>) {
        let _ = device.poll(wgpu::PollType::Poll);
        self.frame = self.frame.saturating_add(1);
        let mut latest = None;
        for slot in &mut self.slots {
            if let Some((frame, mask, receiver)) = &slot.awaiting {
                match receiver.try_recv() {
                    Ok(()) => {
                        let (frame, mask) = (*frame, *mask);
                        let mut encoder =
                            device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: Some("Completed water timestamp resolve"),
                            });
                        Self::resolve_slot(&mut encoder, slot, mask);
                        self.queue.submit([encoder.finish()]);
                        let (send, receive) = mpsc::channel();
                        slot.readback
                            .slice(..)
                            .map_async(wgpu::MapMode::Read, move |result| {
                                let _ = send.send(result);
                            });
                        slot.pending = Some((frame, mask, receive));
                        slot.awaiting = None;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => slot.awaiting = None,
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
            let Some((frame, mask, receiver)) = &slot.pending else {
                continue;
            };
            match receiver.try_recv() {
                Ok(Ok(())) => {
                    let bytes = slot.readback.slice(..).get_mapped_range();
                    let ticks = std::array::from_fn(|i| {
                        u64::from_ne_bytes(bytes[i * 8..i * 8 + 8].try_into().unwrap())
                    });
                    let value = sample(*frame, ticks, *mask, self.period);
                    #[cfg(feature = "diagnostics")]
                    if *frame <= 16 {
                        eprintln!(
                            "water timestamps: frame={frame} mask={mask} period={} ticks={ticks:?} valid={}",
                            self.period,
                            value.is_some()
                        );
                    }
                    if value
                        .is_some_and(|v| latest.is_none_or(|old: GpuSample| v.frame > old.frame))
                    {
                        latest = value;
                    }
                    drop(bytes);
                    slot.readback.unmap();
                    slot.pending = None;
                }
                Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                    slot.readback.unmap();
                    slot.pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        (
            self.slots
                .iter()
                .position(|s| s.pending.is_none() && s.awaiting.is_none()),
            latest,
        )
    }
    pub fn boundary(
        &self,
        slot: Option<usize>,
        pass: u32,
    ) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        let slot = &self.slots[slot?];
        Some(wgpu::RenderPassTimestampWrites {
            query_set: &slot.query,
            beginning_of_pass_write_index: Some(pass * 2),
            end_of_pass_write_index: Some(pass * 2 + 1),
        })
    }
    fn resolve_slot(encoder: &mut wgpu::CommandEncoder, slot: &Slot, mask: u8) {
        // An unwritten query can stall Vulkan or invalidate Metal readback.
        // Every resolve starts at the required 256-byte alignment; copy each
        // written pair to its own position before reusing the resolve buffer.
        for pass in 0..4 {
            if mask & (1 << pass) == 0 {
                continue;
            }
            encoder.resolve_query_set(&slot.query, pass * 2..pass * 2 + 2, &slot.resolve, 0);
            encoder.copy_buffer_to_buffer(
                &slot.resolve,
                0,
                &slot.readback,
                u64::from(pass) * 16,
                16,
            );
        }
    }
    pub fn submitted(&mut self, slot: Option<usize>, mask: u8) {
        if mask & 8 == 0 {
            return;
        }
        let Some(slot) = slot.map(|i| &mut self.slots[i]) else {
            return;
        };
        // Metal can resolve a render-pass end counter before that pass
        // completes. Resolve in a later submission after completion, without
        // waiting on the render thread or reusing the in-flight query slot.
        let (send, receive) = mpsc::channel();
        self.queue.on_submitted_work_done(move || {
            let _ = send.send(());
        });
        slot.awaiting = Some((self.frame, mask, receive));
    }
    pub fn bytes(&self) -> u64 {
        // Eight 64-bit query results, plus equally sized resolve and
        // readback buffers, for each bounded slot. Driver metadata is
        // opaque and is outside declared-resource accounting.
        3 * (64 + 64 + 64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timestamps_charge_overlapping_passes_once_and_exclude_opaque() {
        let result = sample(7, [100, 120, 100, 150, 100, 170, 100, 200], 15, 1e6).unwrap();
        assert_eq!(
            result.pass_ms,
            [Some(20.0), Some(30.0), Some(20.0), Some(30.0)]
        );
        assert_eq!(result.water_ms, 70.0);
        let no_mirror = sample(8, [0, 0, 100, 150, 150, 170, 170, 200], 14, 1e6).unwrap();
        assert_eq!(no_mirror.pass_ms[0], None);
        assert_eq!(no_mirror.water_ms, 50.0);
        let gaps = sample(9, [100, 120, 120, 150, 200, 220, 240, 270], 15, 1e6).unwrap();
        assert_eq!(
            gaps.pass_ms,
            [Some(20.0), Some(30.0), Some(70.0), Some(50.0)]
        );
        assert_eq!(gaps.water_ms, 140.0);
        assert!(sample(9, [0; 8], 8, 1.0).is_none());
        assert!(sample(9, [100, 120, 0, 0, 0, 0, 200, 100], 9, 1.0).is_none());
        assert!(sample(9, [0; 8], 0, 1.0).is_none());
    }
}
