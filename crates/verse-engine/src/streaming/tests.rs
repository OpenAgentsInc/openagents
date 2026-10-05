use super::*;
fn fixture() -> (Manifest, Vec<(String, Vec<u8>)>) {
    let (image, image_bytes) = cook_image(16, 16, &vec![255; 16 * 16 * 4]).unwrap();
    let image_id = image.sha256.clone();
    let mut chunks = BTreeMap::from([(image_id.clone(), image)]);
    let mut data = vec![(image_id.clone(), image_bytes)];
    for index in 0..8 {
        let vertex = Vertex {
            pos: [index as f32, 0., 0.],
            color: [0.3; 3],
            uv: [0.; 2],
            fog: 1.,
        };
        let (descriptor, bytes) = cook_geometry(
            &vec![vertex; 300],
            false,
            vec![image_id.clone()],
            Some(image_id.clone()),
            None,
        )
        .unwrap();
        data.push((descriptor.sha256.clone(), bytes));
        chunks.insert(descriptor.sha256.clone(), descriptor);
    }
    (Manifest { version: 1, chunks }, data)
}
fn finish_source(value: &mut Residency, data: &[(String, Vec<u8>)]) {
    while let Some(ticket) = value.next_source().unwrap() {
        let bytes = data
            .iter()
            .find(|(id, _)| id == ticket.id())
            .unwrap()
            .1
            .clone();
        let decoded = Decoded::decode(bytes, &value.manifest.chunks[ticket.id()]).unwrap();
        value.source_result(ticket, Ok(decoded)).unwrap();
    }
}
fn upload(value: &mut Residency) {
    while let Some(job) = value.next_upload(256) {
        let ticket = job.ticket.clone();
        let offset = job.offset;
        let bytes = job.bytes.len();
        value.uploaded(&ticket, offset, bytes).unwrap();
    }
}
fn budget() -> Budget {
    Budget {
        cpu_bytes: 24 * 1024,
        gpu_bytes: 24 * 1024,
        source_jobs: 2,
        upload_bytes_per_frame: 256,
        upload_ms_per_frame: 1.,
    }
}
#[test]
fn traversal_evicts_and_shared_dependencies_remain_bounded() {
    let (manifest, data) = fixture();
    let mut value = Residency::new(manifest.clone(), budget()).unwrap();
    for (id, _) in data.iter().skip(1) {
        value.request(std::slice::from_ref(id)).unwrap();
        finish_source(&mut value, &data);
        upload(&mut value);
        assert!(value.committed(id));
        assert!(value.committed(&data[0].0));
        assert!(value.metrics().cpu_high_water <= budget().cpu_bytes);
        assert!(value.metrics().gpu_high_water <= budget().gpu_bytes);
    }
    assert!(value.metrics().evictions >= 5);
    assert_eq!(value.metrics().source_starts, 9);
    assert_eq!(value.metrics().jobs_high_water, 1);
    assert!(
        manifest
            .chunks
            .values()
            .map(|v| v.encoded_bytes)
            .sum::<u64>()
            > 3 * budget().cpu_bytes
    );
}
#[test]
fn canceled_jobs_keep_reservations_and_stale_results_cannot_replace_new_zone() {
    let (manifest, data) = fixture();
    let mut value = Residency::new(manifest.clone(), budget()).unwrap();
    value.request(&[data[1].0.clone()]).unwrap();
    let first = value.next_source().unwrap().unwrap();
    let second = value.next_source().unwrap().unwrap();
    let reserved = value.metrics().cpu_bytes;
    value.change_zone(manifest.clone()).unwrap();
    assert_eq!(value.metrics().cpu_bytes, reserved);
    value.request(&[data[2].0.clone()]).unwrap();
    assert!(value.next_source().unwrap().is_none());
    for ticket in [first, second] {
        let bytes = data
            .iter()
            .find(|(id, _)| id == ticket.id())
            .unwrap()
            .1
            .clone();
        let d = Decoded::decode(bytes, &manifest.chunks[ticket.id()]).unwrap();
        value.source_result(ticket, Ok(d)).unwrap();
    }
    assert_eq!(value.metrics().cpu_bytes, 0);
    assert_eq!(value.metrics().stale_results, 2);
    finish_source(&mut value, &data);
    upload(&mut value);
    assert!(value.committed(&data[2].0));
    assert!(!value.committed(&data[1].0));
}
#[test]
fn device_loss_reuploads_retained_verified_sources_and_refuses_old_upload_ticket() {
    let (manifest, data) = fixture();
    let mut value = Residency::new(manifest, budget()).unwrap();
    value.request(&[data[1].0.clone()]).unwrap();
    finish_source(&mut value, &data);
    let job = value.next_upload(256).unwrap();
    let stale = job.ticket.clone();
    let offset = job.offset;
    let bytes = job.bytes.len();
    value.uploaded(&stale, offset, bytes).unwrap();
    let cpu = value.metrics().cpu_bytes;
    value.device_lost().unwrap();
    assert_eq!(value.metrics().cpu_bytes, cpu);
    assert_eq!(value.metrics().gpu_bytes, 0);
    assert!(value.uploaded(&stale, offset, bytes).is_err());
    upload(&mut value);
    assert!(value.committed(&data[1].0));
    assert_eq!(value.metrics().source_starts, 2);
}
#[test]
fn exhausted_views_and_malformed_or_foreign_source_leave_current_content_intact() {
    let (manifest, data) = fixture();
    let mut value = Residency::new(manifest.clone(), budget()).unwrap();
    value.request(&[data[1].0.clone()]).unwrap();
    finish_source(&mut value, &data);
    upload(&mut value);
    assert!(
        value
            .request(
                &data
                    .iter()
                    .skip(1)
                    .map(|(id, _)| id.clone())
                    .collect::<Vec<_>>()
            )
            .is_err()
    );
    assert!(value.committed(&data[1].0));
    value.request(&[data[2].0.clone()]).unwrap();
    let ticket = value.next_source().unwrap().unwrap();
    let foreign = Decoded::decode(data[3].1.clone(), &manifest.chunks[&data[3].0]).unwrap();
    assert!(value.source_result(ticket, Ok(foreign)).is_err());
    assert!(value.next_source().unwrap().is_none());
    assert_eq!(value.metrics().source_failures, 1);
    let mut bytes = data[2].1.clone();
    bytes[32] ^= 1;
    assert!(Decoded::decode(bytes, &manifest.chunks[&data[2].0]).is_err());
    let mut cycle = manifest.clone();
    cycle
        .chunks
        .get_mut(&data[0].0)
        .unwrap()
        .dependencies
        .push(data[1].0.clone());
    assert!(cycle.validate().is_err());
}
#[cfg(all(feature = "asset-io", unix))]
#[test]
fn source_reader_refuses_symlink_and_swapped_digest() {
    use std::os::unix::fs::symlink;
    let (manifest, data) = fixture();
    let root = std::env::temp_dir().join(format!("verse-chunk-reader-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let d = &manifest.chunks[&data[0].0];
    std::fs::write(root.join("other"), &data[0].1).unwrap();
    symlink("other", root.join(format!("{}.vsc", data[0].0))).unwrap();
    assert!(store::Store::open(&root).unwrap().read(d).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejected_shapes_and_ticket_exhaustion_preserve_admitted_state() {
    let (manifest, data) = fixture();
    let mut value = Residency::new(manifest.clone(), budget()).unwrap();
    value.request(&[data[1].0.clone()]).unwrap();
    finish_source(&mut value, &data);
    upload(&mut value);
    value.sequence = u64::MAX - 1;
    let before = value.metrics();
    assert!(value.device_lost().is_err());
    assert_eq!(value.metrics().gpu_bytes, before.gpu_bytes);
    assert!(value.committed(&data[1].0));
    let mut invalid = manifest.chunks[&data[0].0].clone();
    invalid.payload = Kind::Image {
        width: u32::MAX,
        height: u32::MAX,
    };
    assert!(Decoded::decode(Vec::new(), &invalid).is_err());
    let mut bytes = data[1].1.clone();
    bytes[32..36].copy_from_slice(&f32::NAN.to_le_bytes());
    let mut descriptor = manifest.chunks[&data[1].0].clone();
    descriptor.sha256 = super::format::hash(&bytes);
    assert!(Decoded::decode(bytes, &descriptor).is_err());
}
#[cfg(all(feature = "asset-io", not(target_arch = "wasm32")))]
#[test]
fn cooker_publishes_verified_files_and_preserves_existing_digest() {
    let root = tempfile::tempdir().unwrap();
    let (descriptor, bytes) = cook_image(1, 1, &[12, 34, 56, 255]).unwrap();
    let path = store::install(root.path(), &descriptor, &bytes).unwrap();
    store::install(root.path(), &descriptor, &bytes).unwrap();
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    let mut corrupted = bytes.clone();
    corrupted[32] ^= 1;
    std::fs::write(&path, &corrupted).unwrap();
    assert!(store::install(root.path(), &descriptor, &bytes).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), corrupted);
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}
