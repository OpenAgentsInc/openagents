//! Optional GPU spans with bounded, nonblocking delayed readback.
use std::sync::mpsc::{self, Receiver};
pub(super) const FEATURES: wgpu::Features = wgpu::Features::TIMESTAMP_QUERY;
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct Sample {
    pub frame: u64,
    pub shadow_ms: f64,
    pub world_ms: f64,
    pub overlay_ms: f64,
    pub total_ms: f64,
}
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct Health {
    pub issued: u64,
    pub valid: u64,
    pub zero_duration: u64,
    pub out_of_order: u64,
    pub invalid_period: u64,
    pub map_errors: u64,
    pub busy_frames: u64,
    pub batched_completions: u64,
    pub invalid_examples: [Option<[u64; 4]>; 4],
}
#[derive(Debug, PartialEq)]
enum Invalid {
    ZeroDuration,
    OutOfOrder,
    Period,
}
impl Health {
    fn reject(&mut self, reason: Invalid, ticks: [u64; 4]) {
        let count = match reason {
            Invalid::ZeroDuration => &mut self.zero_duration,
            Invalid::OutOfOrder => &mut self.out_of_order,
            Invalid::Period => &mut self.invalid_period,
        };
        *count = count.saturating_add(1);
        if let Some(example) = self
            .invalid_examples
            .iter_mut()
            .find(|value| value.is_none())
        {
            *example = Some(ticks);
        }
    }
}
struct Slot {
    query: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    pending: Option<(u64, bool, Receiver<Result<(), wgpu::BufferAsyncError>>)>,
}
pub(super) struct Timer {
    slots: Vec<Slot>,
    frame: u64,
    period: f64,
    health: Health,
}
fn sample(frame: u64, ticks: [u64; 4], period: f64) -> Result<Sample, Invalid> {
    if !period.is_finite() || period <= 0. {
        return Err(Invalid::Period);
    }
    if ticks.windows(2).any(|pair| pair[1] < pair[0]) {
        return Err(Invalid::OutOfOrder);
    }
    if ticks[3] == ticks[0] {
        return Err(Invalid::ZeroDuration);
    }
    let ms = |a: usize, b: usize| (ticks[b] - ticks[a]) as f64 * period / 1_000_000.;
    Ok(Sample {
        frame,
        shadow_ms: ms(0, 1),
        world_ms: ms(1, 2),
        overlay_ms: ms(2, 3),
        total_ms: ms(0, 3),
    })
}
impl Timer {
    pub fn resume_after(&mut self, frame: u64) {
        self.frame = frame;
    }
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(FEATURES) {
            return None;
        }
        let slots = (0..3)
            .map(|_| Slot {
                query: device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("Verse GPU frame spans"),
                    ty: wgpu::QueryType::Timestamp,
                    count: 4,
                }),
                resolve: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Verse GPU timestamp resolve"),
                    size: 32,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                readback: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Verse delayed GPU timestamps"),
                    size: 32,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                pending: None,
            })
            .collect();
        Some(Self {
            slots,
            frame: 0,
            period: f64::from(queue.get_timestamp_period()),
            health: Health::default(),
        })
    }
    pub fn begin(&mut self, device: &wgpu::Device) -> (Option<usize>, [Option<Sample>; 3]) {
        let _ = device.poll(wgpu::PollType::Poll);
        let mut completed = [None; 3];
        for (index, slot) in self.slots.iter_mut().enumerate() {
            let Some((frame, has_shadow, receiver)) = &slot.pending else {
                continue;
            };
            match receiver.try_recv() {
                Ok(result) => {
                    if result.is_ok() {
                        let bytes = slot.readback.slice(..).get_mapped_range();
                        let mut ticks = std::array::from_fn(|i| {
                            u64::from_ne_bytes(bytes[i * 8..i * 8 + 8].try_into().unwrap())
                        });
                        if !has_shadow {
                            ticks[0] = ticks[1];
                        }
                        match sample(*frame, ticks, self.period) {
                            Ok(value) => {
                                self.health.valid = self.health.valid.saturating_add(1);
                                completed[index] = Some(value);
                            }
                            Err(reason) => self.health.reject(reason, ticks),
                        }
                        drop(bytes);
                        slot.readback.unmap();
                    } else {
                        self.health.map_errors = self.health.map_errors.saturating_add(1);
                    }
                    slot.pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.health.map_errors = self.health.map_errors.saturating_add(1);
                    slot.readback.unmap();
                    slot.pending = None;
                }
            }
        }
        self.frame = self.frame.saturating_add(1);
        let free = self.slots.iter().position(|slot| slot.pending.is_none());
        if free.is_none() {
            self.health.busy_frames = self.health.busy_frames.saturating_add(1);
        }
        if completed.iter().flatten().count() > 1 {
            self.health.batched_completions = self.health.batched_completions.saturating_add(1);
        }
        (free, completed)
    }
    pub fn health(&self) -> Health {
        self.health
    }
    pub fn boundary(
        &self,
        slot: Option<usize>,
        begin: Option<u32>,
        end: Option<u32>,
    ) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        slot.map(|slot| wgpu::RenderPassTimestampWrites {
            query_set: &self.slots[slot].query,
            beginning_of_pass_write_index: begin,
            end_of_pass_write_index: end,
        })
    }
    pub fn resolve(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        slot: Option<usize>,
        has_shadow: bool,
    ) {
        if let Some(slot) = slot {
            let slot = &self.slots[slot];
            // Resolving an unwritten query can wait indefinitely on Vulkan.
            // Query-resolve offsets require 256-byte alignment; shift the copy instead.
            let first = if has_shadow { 0 } else { 1 };
            encoder.resolve_query_set(&slot.query, first..4, &slot.resolve, 0);
            encoder.copy_buffer_to_buffer(
                &slot.resolve,
                0,
                &slot.readback,
                u64::from(first) * 8,
                u64::from(4 - first) * 8,
            );
        }
    }
    pub fn submitted(&mut self, slot: Option<usize>, has_shadow: bool) {
        if let Some(index) = slot {
            self.health.issued = self.health.issued.saturating_add(1);
            let slot = &mut self.slots[index];
            let (tx, rx) = mpsc::channel();
            slot.pending = Some((self.frame, has_shadow, rx));
            slot.readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = tx.send(result);
                });
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "Requires a native GPU; run explicitly when diagnosing timestamp readback"]
    fn native_timestamp_resolve_without_video_capture() {
        native_resolve(true);
    }
    #[test]
    #[ignore = "Requires a native GPU; proves unwritten shadow queries are not resolved"]
    fn native_timestamp_resolve_without_shadow_pass() {
        native_resolve(false);
    }
    fn native_resolve(has_shadow: bool) {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .unwrap();
        assert!(
            adapter.features().contains(super::FEATURES),
            "Adapter lacks pass timestamps"
        );
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: super::FEATURES,
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .unwrap();
        let mut timer = super::Timer::new(&device, &queue).unwrap();
        let (slot, _) = timer.begin(&device);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Verse timestamp probe target"),
            size: wgpu::Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        for first in (if has_shadow { 0 } else { 1 })..4 {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                timestamp_writes: Some(wgpu::RenderPassTimestampWrites {
                    query_set: &timer.slots[slot.unwrap()].query,
                    beginning_of_pass_write_index: Some(first),
                    end_of_pass_write_index: None,
                }),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        timer.resolve(&mut encoder, slot, has_shadow);
        eprintln!("Timestamp probe: submitting");
        let submission = queue.submit([encoder.finish()]);
        timer.submitted(slot, has_shadow);
        eprintln!("Timestamp probe: polling completion");
        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(std::time::Duration::from_secs(5)),
            })
            .unwrap();
        let (_, samples) = timer.begin(&device);
        let sample = samples
            .into_iter()
            .flatten()
            .next()
            .expect("Timestamp readback produces a valid sample");
        assert!(sample.total_ms > 0., "GPU timestamps must advance");
        if !has_shadow {
            assert_eq!(sample.shadow_ms, 0.);
        }
        eprintln!(
            "Timestamp probe: {}",
            serde_json::to_string(&sample).unwrap()
        );
    }
    #[test]
    fn invalid_sample_reasons_are_counted_and_examples_are_bounded() {
        let mut health = super::Health::default();
        for ticks in [[0; 4], [4, 3, 2, 1], [5; 4], [6; 4], [7; 4]] {
            health.reject(super::sample(1, ticks, 1.).unwrap_err(), ticks);
        }
        assert_eq!(health.zero_duration, 4);
        assert_eq!(health.out_of_order, 1);
        assert_eq!(health.invalid_examples.iter().flatten().count(), 4);
        assert_eq!(
            super::sample(1, [0; 4], f64::NAN).unwrap_err(),
            super::Invalid::Period
        );
    }
    #[test]
    fn timestamp_units_and_invalid_order_are_explicit() {
        let s = super::sample(42, [10, 1010, 3010, 4010], 1000.).unwrap();
        assert_eq!(
            (s.frame, s.shadow_ms, s.world_ms, s.overlay_ms, s.total_ms),
            (42, 1., 2., 1., 4.)
        );
        assert!(super::sample(1, [4, 3, 2, 1], 1.).is_err());
        assert!(super::sample(1, [0; 4], f64::NAN).is_err());
        assert!(super::sample(1, [0; 4], 0.).is_err());
        assert!(super::sample(1, [0; 4], 1.).is_err());
    }
}
