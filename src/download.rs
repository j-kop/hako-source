use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Download `url` to `dest` (via a `.part` file). Supports http(s) and file://.
pub fn download(url: &str, dest: &Path) -> Result<()> {
    let part = dest.with_extension("part");
    let result = fetch(url, &part);
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result?;
    std::fs::rename(&part, dest)?;
    Ok(())
}

fn fetch(url: &str, out_path: &Path) -> Result<()> {
    if let Some(p) = url.strip_prefix("file://") {
        std::fs::copy(p, out_path).with_context(|| format!("copying {p}"))?;
        return Ok(());
    }
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        bail!("unsupported URL: {url}");
    }
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("hako/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let resp = client
        .get(url)
        .send()
        .with_context(|| format!("requesting {url}"))?
        .error_for_status()
        .with_context(|| format!("downloading {url}"))?;

    let pb = match resp.content_length() {
        Some(len) => ProgressBar::new(len).with_style(
            ProgressStyle::with_template("  {bar:30} {bytes}/{total_bytes} {bytes_per_sec}")
                .unwrap(),
        ),
        None => ProgressBar::new_spinner(),
    };
    let mut reader = pb.wrap_read(resp);
    let mut file = File::create(out_path)?;
    io::copy(&mut reader as &mut dyn Read, &mut file).context("download interrupted")?;
    pb.finish_and_clear();
    Ok(())
}
