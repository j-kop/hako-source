use crate::manifest::is_safe_relative;
use anyhow::{bail, Context, Result};
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    TarGz,
    TarXz,
    Tar,
    Zip,
    Raw,
}

impl Kind {
    pub fn from_url(url: &str) -> Self {
        let path = url.split(['?', '#']).next().unwrap_or(url).to_ascii_lowercase();
        if path.ends_with(".tar.gz") || path.ends_with(".tgz") {
            Kind::TarGz
        } else if path.ends_with(".tar.xz") || path.ends_with(".txz") {
            Kind::TarXz
        } else if path.ends_with(".tar") {
            Kind::Tar
        } else if path.ends_with(".zip") {
            Kind::Zip
        } else {
            Kind::Raw
        }
    }
}

/// Extract `archive` into `dest`. For `Kind::Raw`, the file is copied to `dest/raw_rel`.
pub fn extract(archive: &Path, kind: Kind, dest: &Path, raw_rel: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    match kind {
        Kind::TarGz => {
            let gz = flate2::read::GzDecoder::new(BufReader::new(File::open(archive)?));
            untar(gz, dest)
        }
        Kind::TarXz => {
            let mut buf = Vec::new();
            lzma_rs::xz_decompress(&mut BufReader::new(File::open(archive)?), &mut buf)
                .map_err(|e| anyhow::anyhow!("xz decompression failed: {e:?}"))?;
            untar(Cursor::new(buf), dest)
        }
        Kind::Tar => untar(BufReader::new(File::open(archive)?), dest),
        Kind::Zip => unzip(archive, dest),
        Kind::Raw => {
            if !is_safe_relative(raw_rel) {
                bail!("unsafe path {}", raw_rel.display());
            }
            let target = dest.join(raw_rel);
            if let Some(p) = target.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(archive, &target)?;
            Ok(())
        }
    }
}

fn untar<R: Read>(reader: R, dest: &Path) -> Result<()> {
    let mut ar = tar::Archive::new(reader);
    ar.set_preserve_permissions(true);
    for entry in ar.entries().context("reading tar archive")? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        // Strip a leading "./" which is common in tarballs.
        let rel = path.strip_prefix(".").unwrap_or(&path);
        if rel.as_os_str().is_empty() {
            continue;
        }
        if !is_safe_relative(rel) {
            bail!("archive contains unsafe path: {}", path.display());
        }
        if !entry.unpack_in(dest)? {
            bail!("archive entry escapes destination: {}", path.display());
        }
    }
    Ok(())
}

fn unzip(archive: &Path, dest: &Path) -> Result<()> {
    let mut za = zip::ZipArchive::new(File::open(archive)?).context("reading zip archive")?;
    for i in 0..za.len() {
        let mut f = za.by_index(i)?;
        let Some(rel) = f.enclosed_name() else {
            bail!("archive contains unsafe path: {}", f.name());
        };
        if !is_safe_relative(&rel) {
            bail!("archive contains unsafe path: {}", f.name());
        }
        let out = dest.join(&rel);
        if f.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut w = File::create(&out)?;
        std::io::copy(&mut f, &mut w)?;
        #[cfg(unix)]
        if let Some(mode) = f.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode & 0o777))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_detection() {
        assert_eq!(Kind::from_url("https://x/a.tar.gz"), Kind::TarGz);
        assert_eq!(Kind::from_url("https://x/a.TGZ?raw=1"), Kind::TarGz);
        assert_eq!(Kind::from_url("https://x/a.tar.xz"), Kind::TarXz);
        assert_eq!(Kind::from_url("https://x/a.zip"), Kind::Zip);
        assert_eq!(Kind::from_url("https://x/hello"), Kind::Raw);
    }

    fn tar_with_name(name: &[u8]) -> Vec<u8> {
        let mut b = tar::Builder::new(Vec::new());
        let mut h = tar::Header::new_gnu();
        h.as_gnu_mut().unwrap().name[..name.len()].copy_from_slice(name);
        h.set_size(4);
        h.set_mode(0o644);
        h.set_entry_type(tar::EntryType::Regular);
        h.set_cksum();
        b.append(&h, &b"evil"[..]).unwrap();
        b.into_inner().unwrap()
    }

    #[test]
    fn rejects_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out");
        for bad in [&b"../evil"[..], b"/tmp/evil", b"a/../../evil"] {
            let ar = dir.path().join("bad.tar");
            std::fs::write(&ar, tar_with_name(bad)).unwrap();
            let err = extract(&ar, Kind::Tar, &dest, Path::new("x")).unwrap_err();
            assert!(err.to_string().contains("unsafe"), "{err}");
        }
        assert!(!dir.path().join("evil").exists());
    }

    #[test]
    fn extracts_normal_tar() {
        let dir = tempfile::tempdir().unwrap();
        let ar = dir.path().join("ok.tar");
        std::fs::write(&ar, tar_with_name(b"./pkg/hello")).unwrap();
        let dest = dir.path().join("out");
        extract(&ar, Kind::Tar, &dest, Path::new("x")).unwrap();
        assert_eq!(std::fs::read(dest.join("pkg/hello")).unwrap(), b"evil");
    }
}
