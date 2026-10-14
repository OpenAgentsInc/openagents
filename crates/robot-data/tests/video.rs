#![cfg(feature = "video")]

use std::time::Duration;

use robot_data::replay::{CameraAt, CameraStream};
use robot_data::video::{CameraFrame, Decoder};
use supervise::{Job, Limits};

#[tokio::test]
async fn supervised_local_decode_returns_the_selected_frame_and_missing_video_is_blank() {
    let root = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let path = root.path().join("camera.mp4");
    let ended = Job::new("ffmpeg")
        .args([
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=red:size=16x16:rate=10:duration=0.4",
            "-c:v",
            "mpeg4",
            "-threads",
            "1",
            "-y",
        ])
        .arg(path.as_os_str())
        .bounded(
            Limits::within(Duration::from_secs(10))
                .keeping(4_096)
                .memory(Some(512 << 20)),
        )
        .run()
        .await;
    assert_eq!(
        ended.ending.code(),
        Some(0),
        "{}: {}",
        ended.ending,
        ended.stderr.text
    );
    let decoder = Decoder::new(root.path(), scratch.path()).unwrap();
    let stream = CameraStream {
        file: "camera.mp4".into(),
        from_seconds: 0.0,
        to_seconds: 1.0,
        width: 16,
        height: 16,
        fps: 10.0,
    };
    let selected = CameraAt::Frame {
        file: "camera.mp4".into(),
        frame: 2,
        timestamp: 0.2,
    };
    let CameraFrame::Rgba {
        bytes,
        width,
        height,
    } = decoder.decode(&stream, &selected).await.unwrap()
    else {
        panic!("Expected a camera frame")
    };
    assert_eq!((width, height, bytes.len()), (16, 16, 16 * 16 * 4));
    assert!(
        bytes
            .chunks_exact(4)
            .all(|pixel| pixel[0] > 200 && pixel[1] < 20 && pixel[2] < 20 && pixel[3] == 255)
    );
    assert_eq!(
        decoder.decode(&stream, &CameraAt::Missing).await.unwrap(),
        CameraFrame::Missing
    );
    assert_eq!(
        decoder
            .decode(
                &stream,
                &CameraAt::Frame {
                    file: "camera.mp4".into(),
                    frame: 9,
                    timestamp: 0.9
                }
            )
            .await
            .unwrap(),
        CameraFrame::Missing
    );
    std::fs::remove_file(path).unwrap();
    assert_eq!(
        decoder.decode(&stream, &selected).await.unwrap(),
        CameraFrame::Missing
    );
    assert_eq!(
        std::fs::read_dir(scratch.path()).unwrap().count(),
        0,
        "Decoder releases temporary frames"
    );
}
