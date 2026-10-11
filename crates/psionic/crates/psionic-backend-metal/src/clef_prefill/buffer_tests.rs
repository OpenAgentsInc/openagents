//! Device checks for buffer copies and command ownership.

use super::*;

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn device_copies_use_view_offsets_and_reject_overlap_before_dispatch() {
    objc::rc::autoreleasepool(|| {
        let metal = ClefMetal::new().expect("Metal");
        let source = metal.buffer_f32(&(0..600).map(|i| i as f32).collect::<Vec<_>>());
        let destination = metal.buffer_f32(&vec![-1.0; 600]);
        let a = source.view(16, 1100).unwrap();
        let b = destination.view(32, 1100).unwrap();
        let batch = metal.batch();
        assert!(batch.copy_f32(&a, 0, &a, 1, 10).is_err());
        assert!(batch.copy_f32(&a, 0, &b, 0, 276).is_err());
        assert!(batch.copy_f32(&a, usize::MAX, &b, 0, 1).is_err());
        batch.copy_f32(&a, 1, &b, 2, 257).unwrap();
        batch
            .copy_f32(&destination, 10, &destination, 400, 100)
            .unwrap();
        batch.commit_wait().unwrap();
        let got = destination.read_f32(0, 600).unwrap();
        assert!(got[..10].iter().all(|x| *x == -1.0));
        assert_eq!(got[10..267], (5..262).map(|i| i as f32).collect::<Vec<_>>());
        assert!(got[267..400].iter().all(|x| *x == -1.0));
        assert_eq!(got[400..500], got[10..110]);
        assert!(got[500..].iter().all(|x| *x == -1.0));
    });
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn buffer_views_keep_ownership_and_bound_host_and_kernel_writes() {
    objc::rc::autoreleasepool(|| {
        let metal = ClefMetal::new().expect("Metal");
        let source = metal.buffer_f32(&vec![1.0; 512]).view(64, 1024).unwrap();
        // The source view outlives its original buffer handle.
        let parent = metal.buffer_with_bytes(&vec![0xa5; 576]);
        let destination = parent.view(32, 512).unwrap();
        let batch = metal.batch();
        batch
            .dequantize_to_f16(ClefMetalWeightFormat::F32, &source, &destination, 256)
            .unwrap();
        batch.commit_wait().unwrap();
        assert_eq!(
            destination.read_bytes(0, 512).unwrap(),
            [0, 0x3c].repeat(256)
        );
        assert_eq!(parent.read_bytes(0, 32).unwrap(), vec![0xa5; 32]);
        assert_eq!(parent.read_bytes(544, 32).unwrap(), vec![0xa5; 32]);
        let nested = destination.view(16, 16).unwrap();
        nested.write_f32(1, &[3.0]).unwrap();
        assert_eq!(parent.read_f32(13, 1).unwrap(), vec![3.0]);
        nested.zero();
        assert_eq!(parent.read_bytes(48, 16).unwrap(), vec![0; 16]);
        assert_eq!(destination.read_bytes(0, 16).unwrap(), [0, 0x3c].repeat(8));
        assert!(destination.view(1, 16).is_err());
        assert!(destination.view(512, 1).is_err());
        assert!(destination.view(usize::MAX, 1).is_err());
        assert!(nested.read_f32(usize::MAX, 1).is_err());
        assert!(nested.read_f32(0, usize::MAX).is_err());
        assert!(nested.write_f32(usize::MAX, &[0.0]).is_err());
    });
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn byte_uploads_and_snapshots_preserve_short_and_unaligned_ranges() {
    objc::rc::autoreleasepool(|| {
        let metal = ClefMetal::new().expect("Metal");
        for length in [0, 1, 3, 15, 16, 17, 65] {
            let bytes: Vec<u8> = (0..length).map(|i| (i * 13) as u8).collect();
            let source = metal.buffer_with_bytes(&bytes);
            assert_eq!(source.read_bytes(0, length).unwrap(), bytes);
            if length < 16 {
                assert_eq!(
                    source.read_bytes(length, 16 - length).unwrap(),
                    vec![0; 16 - length]
                );
            }
            let destination = metal.buffer(length + 19);
            destination
                .write_bytes(3, &source.read_bytes(0, length).unwrap())
                .unwrap();
            assert_eq!(destination.read_bytes(3, length).unwrap(), bytes);
            assert_eq!(destination.read_bytes(0, 3).unwrap(), vec![0; 3]);
            assert!(source.read_bytes(usize::MAX, 1).is_err());
            assert!(source.read_bytes(0, source.byte_len() + 1).is_err());
        }
    });
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn completed_batches_release_buffers_before_the_outer_pool_drains() {
    // Keep one outer pool alive through the assertion: relying on a caller's
    // pool would retain all eight pairs of temporary buffers until it drains.
    objc::rc::autoreleasepool(|| {
        let metal = ClefMetal::new().expect("Metal");
        let before = metal.device.current_allocated_size();
        for _ in 0..8 {
            let elements = 1024 * 1024;
            let source = metal.buffer(elements * 4);
            let destination = metal.buffer(elements * 2);
            let batch = metal.batch();
            batch
                .dequantize_to_f16(ClefMetalWeightFormat::F32, &source, &destination, elements)
                .unwrap();
            batch.commit_wait().unwrap();
        }
        // Allow the driver to keep its most recent completed submission.
        metal.batch().commit_wait().unwrap();
        let after = metal.device.current_allocated_size();
        assert!(
            after <= before + 16 * 1024 * 1024,
            "temporary Metal bytes: before={before}, after={after}"
        );
    });
}
