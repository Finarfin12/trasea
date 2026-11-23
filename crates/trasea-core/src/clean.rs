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

/// Remove the Windows verbatim prefix that Tauri's resource_dir() and canonicalize() add (a backslash pair, a question
/// mark, a backslash). realesrgan-ncnn-vulkan glues paths together as text, so a path with that prefix makes it fail to
/// open its model and crash with 0xC0000409.
pub fn plain_path(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    let unc = [r"\\?\UNC\"];
    for pre in unc {
        if let Some(rest) = s.strip_prefix(pre) {
            return PathBuf::from(format!(r"\\{rest}"));
        }
    }
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        return PathBuf::from(rest);
    }
    p.to_path_buf()
}

fn locate(dir: &Path) -> Option<(PathBuf, PathBuf)> {
    let dir = plain_path(dir);
    let exe = dir.join(SIDECAR_EXE);
    let models = dir.join("models");
    (exe.is_file() && models.is_dir()).then_some((exe, models))
}

/// Run the sidecar on `img`. Err(reason) when it cannot run.
fn run_sidecar(img: &RgbImage, dir: &Path, up: u32) -> Result<RgbImage, String> {
    let (exe, models) = locate(dir).ok_or_else(|| format!("{SIDECAR_EXE} or its models folder not found in {}", dir.display()))?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let tmp = plain_path(&std::env::temp_dir()).join(format!("trasea-{}-{}", std::process::id(), nanos));
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
        // Some GPU/driver setups make the first try crash; retry with a small tile size and each Vulkan device.
        let attempts: [&[&str]; 3] = [&[], &["-t", "128"], &["-g", "1", "-t", "128"]];
        let mut last_err = String::new();
        let mut done = false;
        for (n, extra) in attempts.iter().enumerate() {
            let _ = std::fs::remove_file(&outp);
            let log = tmp.join(format!("sidecar{n}.log"));
            let log_file = std::fs::File::create(&log).map_err(|e| e.to_string())?;
            let log_err = log_file.try_clone().map_err(|e| e.to_string())?;
            let mut cmd = Command::new(&exe);
            cmd.arg("-i").arg(&inp).arg("-o").arg(&outp).args(["-n", MODEL, "-s", "4", "-m"]).arg(&models).args(*extra).stdout(Stdio::from(log_file)).stderr(Stdio::from(log_err)).stdin(Stdio::null());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
            }
            let mut child = cmd.spawn().map_err(|e| format!("cannot start the Real-ESRGAN sidecar: {e}"))?;
            let t0 = Instant::now();
            loop {
                match child.try_wait().map_err(|e| e.to_string())? {
                    Some(status) if status.success() && outp.is_file() => {
                        done = true;
                        break;
                    }
                    Some(status) => {
                        let tail = std::fs::read_to_string(&log).unwrap_or_default().lines().rev().find(|l| !l.trim().is_empty() && !l.contains('%')).unwrap_or("").to_string();
                        last_err = format!("the Real-ESRGAN sidecar failed ({status}) {tail}");
                        break;
                    }
                    None if t0.elapsed() > TIMEOUT => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err("the Real-ESRGAN sidecar took too long and was stopped".into());
                    }
                    None => std::thread::sleep(Duration::from_millis(100)),
                }
            }
            if done {
                break;
            }
        }
        if !done {
            return Err(last_err);
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
    let (why, runtime_gpu_fail) = match sidecar_dir {
        Some(dir) => match run_sidecar(img, dir, up) {
            Ok(image) => return Cleaned { image, used_ai: true, note: None },
            Err(why) => {
                let missing = why.contains("not found");
                (why, !missing)
            }
        },
        None => ("the Real-ESRGAN sidecar is not available".to_string(), false),
    };
    let note = if runtime_gpu_fail {
        format!("Pembersih AI (Vulkan) tidak tersedia di kartu grafis ini — Trasea otomatis memakai pembersih cepat lokal (Median). ({why})")
    } else {
        format!("AI cleaning was skipped ({why}). A median filter was used instead.")
    };
    Cleaned { image: fallback(img, up), used_ai: false, note: Some(note) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbatim_prefix_is_removed() {
        assert_eq!(plain_path(Path::new(r"\\?\C:\Users\x\realesrgan")), PathBuf::from(r"C:\Users\x\realesrgan"));
        assert_eq!(plain_path(Path::new(r"\\?\UNC\server\share\a")), PathBuf::from(r"\\server\share\a"));
        assert_eq!(plain_path(Path::new(r"C:\plain\path")), PathBuf::from(r"C:\plain\path"));
    }

    #[test]
    fn locate_accepts_a_folder_with_the_verbatim_prefix() {
        let dir = std::env::temp_dir().join("trasea-locate-test");
        std::fs::create_dir_all(dir.join("models")).unwrap();
        std::fs::write(dir.join(SIDECAR_EXE), b"x").unwrap();
        let verbatim = PathBuf::from(format!("{}{}", r"\\?\", dir.display()));
        let (exe, models) = locate(&verbatim).expect("a sidecar folder with a verbatim prefix must be found");
        for p in [&exe, &models] {
            assert!(!p.to_string_lossy().starts_with(r"\\?\"), "still verbatim: {}", p.display());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
