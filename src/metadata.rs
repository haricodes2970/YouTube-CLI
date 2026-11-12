use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::process::Command;

/// Video metadata extracted via FFprobe.
#[derive(Debug, Clone, Deserialize)]
pub struct VideoMetadata {
    pub file_path: String,
    pub width: u32,
    pub height: u32,
    pub codec: String,
    pub duration_secs: f64,
    pub frame_rate: f64,
    pub pixel_format: String,
}

/// Raw FFprobe JSON stream entry.
#[derive(Debug, Deserialize)]
struct FfprobeStream {
    #[allow(dead_code)]
    codec_type: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    codec_name: Option<String>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
    duration: Option<String>,
    pix_fmt: Option<String>,
}

/// Raw FFprobe JSON output.
#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    streams: Vec<FfprobeStream>,
    format: Option<FfprobeFormat>,
}

/// Raw FFprobe format entry.
#[derive(Debug, Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
}

/// Parse a fraction like "30/1" or "25/1" into an f64.
fn parse_fraction(value: &str) -> Option<f64> {
    let parts: Vec<&str> = value.split('/').collect();
    match parts.as_slice() {
        [num] => num.parse::<f64>().ok(),
        [num, den] => {
            let n = num.parse::<f64>().ok()?;
            let d = den.parse::<f64>().ok()?;
            if d == 0.0 {
                None
            } else {
                Some(n / d)
            }
        }
        _ => None,
    }
}

/// Extract video metadata from a local file using FFprobe as a subprocess.
pub fn probe(path: &str) -> Result<VideoMetadata> {
    // Verify the file exists and is readable before invoking FFprobe.
    std::fs::metadata(path).map_err(|e| anyhow!("cannot access file '{path}': {e}"))?;

    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg("stream=width,height,codec_name,avg_frame_rate,r_frame_rate,duration,pix_fmt")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("json")
        .arg(path)
        .output()
        .map_err(|e| anyhow!("failed to spawn ffprobe: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("ffprobe failed for '{path}': {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let data: FfprobeOutput = serde_json::from_str(&stdout)
        .map_err(|e| anyhow!("failed to parse ffprobe output: {e}"))?;

    let stream = data
        .streams
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no video stream found in '{path}'"))?;

    let width = stream
        .width
        .ok_or_else(|| anyhow!("ffprobe did not report width for '{path}'"))?;
    let height = stream
        .height
        .ok_or_else(|| anyhow!("ffprobe did not report height for '{path}'"))?;
    let codec = stream.codec_name.unwrap_or_else(|| "unknown".to_string());

    // Prefer stream duration, fall back to format duration.
    let duration_secs = stream
        .duration
        .and_then(|d| d.parse::<f64>().ok())
        .or_else(|| {
            data.format
                .and_then(|f| f.duration)
                .and_then(|d| d.parse::<f64>().ok())
        })
        .unwrap_or(0.0);

    let frame_rate = stream
        .avg_frame_rate
        .or(stream.r_frame_rate)
        .and_then(|f| parse_fraction(&f))
        .unwrap_or(0.0);

    let pixel_format = stream.pix_fmt.unwrap_or_else(|| "unknown".to_string());

    Ok(VideoMetadata {
        file_path: path.to_string(),
        width,
        height,
        codec,
        duration_secs,
        frame_rate,
        pixel_format,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fraction_handles_integers() {
        assert_eq!(parse_fraction("30"), Some(30.0));
        assert_eq!(parse_fraction("25"), Some(25.0));
    }

    #[test]
    fn parse_fraction_handles_ratios() {
        assert_eq!(parse_fraction("30/1"), Some(30.0));
        assert_eq!(parse_fraction("25/1"), Some(25.0));
        assert_eq!(parse_fraction("30000/1001"), Some(29.97002997002997));
    }

    #[test]
    fn parse_fraction_rejects_invalid() {
        assert_eq!(parse_fraction(""), None);
        assert_eq!(parse_fraction("abc"), None);
        assert_eq!(parse_fraction("1/0"), None);
        assert_eq!(parse_fraction("a/b"), None);
    }

    #[test]
    fn probe_nonexistent_file_returns_error() {
        let result = probe("/tmp/does-not-exist-youtube-cli-test.mp4");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("cannot access"));
    }
}
