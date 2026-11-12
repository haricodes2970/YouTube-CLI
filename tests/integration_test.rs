use std::process::Command;

use youtube_cli::decoder::Decoder;
use youtube_cli::metadata::probe;
use youtube_cli::renderer::VideoFrame;

fn make_test_video(path: &str) {
    let _ = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("testsrc=duration=2:size=80x60:rate=5")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg(path)
        .output();
}

#[test]
fn ffprobe_metadata_parsing() {
    let path = "/tmp/youtube-cli-metadata-test.mp4";
    make_test_video(path);

    let meta = probe(path).expect("ffprobe should succeed");
    assert_eq!(meta.width, 80);
    assert_eq!(meta.height, 60);
    assert!(!meta.codec.is_empty());
    assert!(meta.duration_secs > 0.0);
    assert!(meta.frame_rate > 0.0);
    assert!(!meta.pixel_format.is_empty());

    println!("Metadata: {meta:?}");
}

#[test]
fn ffprobe_invalid_file_returns_error() {
    let result = probe("/tmp/nonexistent-youtube-cli-test-file.mp4");
    assert!(result.is_err());
}

#[test]
fn rgb24_frame_reading() {
    let path = "/tmp/youtube-cli-decode-test.mp4";
    make_test_video(path);

    let meta = probe(path).expect("ffprobe should succeed");
    let mut dec = Decoder::spawn(path, meta.width, meta.height).expect("decoder should spawn");

    assert_eq!(dec.width(), meta.width);
    assert_eq!(dec.height(), meta.height);
    assert_eq!(
        dec.frame_size(),
        meta.width as usize * meta.height as usize * 3
    );

    let mut frame_count = 0;
    while let Some(raw) = dec.next_frame().expect("frame read should not error") {
        assert_eq!(raw.len(), dec.frame_size());
        frame_count += 1;
    }

    assert!(frame_count > 0, "should decode at least one frame");
    assert!(dec.eof(), "should reach EOF");
    println!("Decoded {frame_count} frames from {path}");
}

#[test]
fn eof_handling() {
    let path = "/tmp/youtube-cli-eof-test.mp4";
    make_test_video(path);

    let meta = probe(path).expect("ffprobe should succeed");
    let mut dec = Decoder::spawn(path, meta.width, meta.height).expect("decoder should spawn");

    // Read all frames until EOF
    while dec.next_frame().expect("read should not error").is_some() {}

    assert!(dec.eof());
    // After EOF, next_frame returns None
    assert!(dec.next_frame().unwrap().is_none());
}

#[test]
fn frame_buffer_validation() {
    // Valid frame
    let valid = VideoFrame {
        width: 2,
        height: 2,
        pixels: vec![0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255],
        timestamp_ms: 0,
    };
    assert!(valid.validate().is_ok());

    // Too small
    let too_small = VideoFrame {
        width: 2,
        height: 2,
        pixels: vec![0, 0, 0],
        timestamp_ms: 0,
    };
    assert!(too_small.validate().is_err());

    // Too large
    let too_large = VideoFrame {
        width: 2,
        height: 2,
        pixels: vec![0; 13],
        timestamp_ms: 0,
    };
    assert!(too_large.validate().is_err());
}

#[test]
fn decoder_shutdown() {
    let path = "/tmp/youtube-cli-shutdown-test.mp4";
    make_test_video(path);

    let meta = probe(path).expect("ffprobe should succeed");
    let mut dec = Decoder::spawn(path, meta.width, meta.height).expect("decoder should spawn");

    // Read a few frames then terminate
    for _ in 0..3 {
        let _ = dec.next_frame().expect("read should not error");
    }

    dec.terminate().expect("terminate should succeed");
    assert!(dec.eof());
}
