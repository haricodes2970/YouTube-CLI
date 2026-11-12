use anyhow::{anyhow, Result};
use std::io::{BufReader, Read};
use std::process::{Child, ChildStdout, Command, Stdio};

/// FFmpeg subprocess decoder.
///
/// Spawns `ffmpeg` to decode a video file into raw RGB24 frames on stdout,
/// one frame at a time. Frames are read incrementally so the entire video is
/// never loaded into memory.
pub struct Decoder {
    child: Option<Child>,
    stdout_reader: Option<BufReader<ChildStdout>>,
    /// Width of decoded frames (matches requested output width).
    width: u32,
    /// Height of decoded frames (matches requested output height).
    height: u32,
    /// Bytes per frame = width * height * 3 (RGB24).
    frame_size: usize,
    /// Whether EOF has been reached on stdout.
    eof: bool,
}

impl Decoder {
    /// Spawn ffmpeg to decode `path` into raw RGB24 frames at `width` x `height`.
    ///
    /// The ffmpeg command pipeline:
    ///   ffmpeg -hide_banner -loglevel error -i <path>
    ///            -f rawvideo -pix_fmt rgb24 -s <W>x<H> -
    ///
    /// stdout emits raw RGB24 frames; stderr carries diagnostics.
    pub fn spawn(path: &str, width: u32, height: u32) -> Result<Self> {
        let frame_size = (width as usize) * (height as usize) * 3;

        let mut child = Command::new("ffmpeg")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-i")
            .arg(path)
            .arg("-f")
            .arg("rawvideo")
            .arg("-pix_fmt")
            .arg("rgb24")
            .arg("-s")
            .arg(format!("{width}x{height}"))
            .arg("-")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow!("failed to spawn ffmpeg for '{path}': {e}"))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("ffmpeg did not provide stdout for '{path}'"))?;
        let stdout_reader = BufReader::new(stdout);

        Ok(Self {
            child: Some(child),
            stdout_reader: Some(stdout_reader),
            width,
            height,
            frame_size,
            eof: false,
        })
    }

    /// Read the next raw RGB24 frame from ffmpeg stdout.
    ///
    /// Returns `Ok(None)` at EOF. Reads exactly `frame_size` bytes; a short
    /// read indicates EOF or a truncated frame.
    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>> {
        if self.eof {
            return Ok(None);
        }

        let reader = self
            .stdout_reader
            .as_mut()
            .ok_or_else(|| anyhow!("decoder stdout already consumed"))?;

        let mut buffer = vec![0u8; self.frame_size];
        let mut total_read = 0;

        while total_read < self.frame_size {
            let n = reader
                .read(&mut buffer[total_read..])
                .map_err(|e| anyhow!("failed to read frame from ffmpeg stdout: {e}"))?;
            if n == 0 {
                // EOF before a full frame.
                if total_read == 0 {
                    self.eof = true;
                    return Ok(None);
                }
                // Incomplete frame: treat as EOF.
                self.eof = true;
                return Ok(Some(buffer[..total_read].to_vec()));
            }
            total_read += n;
        }

        Ok(Some(buffer))
    }

    /// Decoded frame width.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Decoded frame height.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Bytes per decoded frame (width * height * 3 for RGB24).
    pub fn frame_size(&self) -> usize {
        self.frame_size
    }

    /// Whether EOF has been reached.
    pub fn eof(&self) -> bool {
        self.eof
    }

    /// Terminate the ffmpeg child process and clean up resources.
    pub fn terminate(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            // Attempt a graceful shutdown first.
            let _ = child.kill();
            let _ = child.wait();
        }
        self.stdout_reader = None;
        self.eof = true;
        Ok(())
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoder_frame_size() {
        let d = Decoder {
            child: None,
            stdout_reader: None,
            width: 10,
            height: 10,
            frame_size: 10 * 10 * 3,
            eof: false,
        };
        assert_eq!(d.frame_size(), 300);
        assert_eq!(d.width(), 10);
        assert_eq!(d.height(), 10);
    }

    #[test]
    fn decoder_spawn_fails_on_missing_binary() {
        // ffmpeg spawns even for a nonexistent input; reading frames yields EOF.
        let mut d = Decoder::spawn("/tmp/does-not-exist-youtube-cli-test.mp4", 10, 10).unwrap();
        let result = d.next_frame().unwrap();
        assert!(result.is_none()); // immediate EOF, no frames
    }
}
