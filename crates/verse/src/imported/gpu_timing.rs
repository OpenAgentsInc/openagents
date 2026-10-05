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
}
fn sample(frame: u64, ticks: [u64; 4], period: f64) -> Option<Sample> {
    if ticks[3] == ticks[0]
        || !period.is_finite()
        || period <= 0.
        || ticks.windows(2).any(|pair| pair[1] < pair[0])
    {
        return None;
    }
    let ms = |a: usize, b: usize| (ticks[b] - ticks[a]) as f64 * period / 1_000_000.;
    Some(Sample {
        frame,
        shadow_ms: ms(0, 1),
        world_ms: ms(1, 2),
        overlay_ms: ms(2, 3),
        total_ms: ms(0, 3),
    })
}
impl Timer {
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
        })
    }
    pub fn begin(&mut self, device: &wgpu::Device) -> (Option<usize>, Option<Sample>) {
        let _ = device.poll(wgpu::PollType::Poll);
        let mut latest: Option<Sample> = None;
        for slot in &mut self.slots {
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
                        if let Some(value) = sample(*frame, ticks, self.period)
                            && latest.is_none_or(|previous| previous.frame < value.frame)
                        {
                            latest = Some(value);
                        }
                        drop(bytes);
                        slot.readback.unmap();
                    }
                    slot.pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    slot.readback.unmap();
                    slot.pending = None;
                }
            }
        }
        self.frame = self.frame.saturating_add(1);
        (
            self.slots.iter().position(|slot| slot.pending.is_none()),
            latest,
        )
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
    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder, slot: Option<usize>) {
        if let Some(slot) = slot {
            let slot = &self.slots[slot];
            encoder.resolve_query_set(&slot.query, 0..4, &slot.resolve, 0);
            encoder.copy_buffer_to_buffer(&slot.resolve, 0, &slot.readback, 0, 32);
        }
    }
    pub fn submitted(&mut self, slot: Option<usize>, has_shadow: bool) {
        if let Some(index) = slot {
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
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .unwrap();
        assert!(
            adapter.features().contains(super::FEATURES),
            "Adapter lacks encoder timestamps"
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
        for first in [0, 2] {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                timestamp_writes: Some(wgpu::RenderPassTimestampWrites {
                    query_set: &timer.slots[slot.unwrap()].query,
                    beginning_of_pass_write_index: Some(first),
                    end_of_pass_write_index: Some(first + 1),
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
        timer.resolve(&mut encoder, slot);
        eprintln!("Timestamp probe: submitting");
        let submission = queue.submit([encoder.finish()]);
        timer.submitted(slot, true);
        eprintln!("Timestamp probe: polling completion");
        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(std::time::Duration::from_secs(5)),
            })
            .unwrap();
        let (_, sample) = timer.begin(&device);
        let sample = sample.expect("Timestamp readback produces a valid sample");
        assert!(sample.total_ms > 0., "GPU timestamps must advance");
        eprintln!(
            "Timestamp probe: {}",
            serde_json::to_string(&sample).unwrap()
        );
    }
    #[test]
    fn timestamp_units_and_invalid_order_are_explicit() {
        let s = super::sample(42, [10, 1010, 3010, 4010], 1000.).unwrap();
        assert_eq!(
            (s.frame, s.shadow_ms, s.world_ms, s.overlay_ms, s.total_ms),
            (42, 1., 2., 1., 4.)
        );
        assert!(super::sample(1, [4, 3, 2, 1], 1.).is_none());
        assert!(super::sample(1, [0; 4], f64::NAN).is_none());
        assert!(super::sample(1, [0; 4], 0.).is_none());
        assert!(super::sample(1, [0; 4], 1.).is_none());
    }
}
