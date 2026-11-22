//! Clean stage: Real-ESRGAN (anime x4) sidecar, downsampled to `up`x; median fallback when the sidecar or a GPU is missing.

use image::{imageops::FilterType, RgbImage};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct Cleaned {
    pub image: RgbImage,
    /// true when Real-ESRGAN produced the image.
    pub used_ai: bool,
    /// Message for the user when the AI step was wanted but could not run.
    pub note: Option<String>,
}

pub const SIDECAR_EXE: &str = "realesrgan-ncnn-vulkan.exe";
pub const MODEL: &str = "realesrgan-x4plus-anime";
/// The network is fed at most this long side (speed on integrated GPUs); the result is scaled to `up`x anyway.
const ESRGAN_MAX_INPUT: u32 = 1200;
const TIMEOUT: Duration = Duration::from_secs(240);

/// Median filter with a 3x3 window on each channel.
pub fn median3(img: &RgbImage) -> RgbImage {
    let (w, h) = img.dimensions();
    let mut out = RgbImage::new(w, h);
    let mut win = [0u8; 9];
    for y in 0..h {
        for x in 0..w {
            let mut px = [0u8; 3];
            for c in 0..3 {
                let mut n = 0;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let xx = (x as i32 + dx).clamp(0, w as i32 - 1) as u32;
                        let yy = (y as i32 + dy).clamp(0, h as i32 - 1) as u32;
                        win[n] = img.get_pixel(xx, yy)[c];
                        n += 1;
                    }
                }
                win.sort_unstable();
                px[c] = win[4];
            }
            out.put_pixel(x, y, image::Rgb(px));
        }
    }
    out
}

fn fallback(img: &RgbImage, up: u32) -> RgbImage {
    let (w, h) = img.dimensions();
    let big = image::imageops::resize(img, w * up, h * up, FilterType::CatmullRom);
    median3(&big)
}

fn locate(dir: &Path) -> Option<(PathBuf, PathBuf)> {
    let exe = dir.join(SIDECAR_EXE);
    let models = dir.join("models");
    (exe.is_file() && models.is_dir()).then_some((exe, models))
}

/// Run the sidecar on `img`. Err(reason) when it cannot run.
fn run_sidecar(img: &RgbImage, dir: &Path, up: u32) -> Result<RgbImage, String> {
    let (exe, models) = locate(dir).ok_or_else(|| format!("{SIDECAR_EXE} or its models folder not found in {}", dir.display()))?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let tmp = std::env::temp_dir().join(format!("vektora-{}-{}", std::process::id(), nanos));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let result = (|| {
        let (w, h) = img.dimensions();
        let long = w.max(h);
        let input: std::borrow::Cow<RgbImage> = if long > ESRGAN_MAX_INPUT {
            let f = ESRGAN_MAX_INPUT as f32 / long as f32;
            std::borrow::Cow::Owned(image::imageops::resize(img, (w as f32 * f).round() as u32, (h as f32 * f).round() as u32, FilterType::Lanczos3))
        } else {
            std::borrow::Cow::Borrowed(img)
        };
        let inp = tmp.join("in.png");
        let outp = tmp.join("out.png");
        input.save(&inp).map_err(|e| e.to_string())?;
        let mut cmd = Command::new(&exe);
        cmd.arg("-i").arg(&inp).arg("-o").arg(&outp).args(["-n", MODEL, "-s", "4", "-m"]).arg(&models).stdout(Stdio::null()).stderr(Stdio::null()).stdin(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd.spawn().map_err(|e| format!("cannot start the Real-ESRGAN sidecar: {e}"))?;
        let t0 = Instant::now();
        loop {
            match child.try_wait().map_err(|e| e.to_string())? {
                Some(status) if status.success() => break,
                Some(status) => return Err(format!("the Real-ESRGAN sidecar failed ({status}); probably no usable GPU or Vulkan driver")),
                None if t0.elapsed() > TIMEOUT => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("the Real-ESRGAN sidecar took too long and was stopped".into());
                }
                None => std::thread::sleep(Duration::from_millis(100)),
            }
        }
        let out = image::open(&outp).map_err(|e| format!("cannot read the sidecar output: {e}"))?.to_rgb8();
        Ok(image::imageops::resize(&out, w * up, h * up, FilterType::Lanczos3))
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

/// Produce the clean working image at `up`x the size of `img`.
pub fn clean(img: &RgbImage, want_ai: bool, sidecar_dir: Option<&Path>, up: u32) -> Cleaned {
    if !want_ai {
        return Cleaned { image: fallback(img, up), used_ai: false, note: None };
    }
    let why = match sidecar_dir {
        Some(dir) => match run_sidecar(img, dir, up) {
            Ok(image) => return Cleaned { image, used_ai: true, note: None },
            Err(why) => why,
        },
        None => "the Real-ESRGAN sidecar is not available".to_string(),
    };
    Cleaned { image: fallback(img, up), used_ai: false, note: Some(format!("AI cleaning was skipped ({why}). A median filter was used instead.")) }
}
