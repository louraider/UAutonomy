//! Встановлені пакети на диску.
//!
//! ```text
//! <root>/packs/<id>/manifest.json     встановлений пакет
//! <root>/packs/<id>/<файли>
//! <root>/downloads/<id>/<файли>.part  незавершені завантаження (докачуються)
//! <root>/staging/                     тимчасові теки під час імпорту
//! ```

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::pack::{is_safe_rel_path, PackFile, PackManifest};
use crate::Error;

#[derive(Debug, Clone)]
pub struct PackStore {
    root: PathBuf,
}

impl PackStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, Error> {
        let root = root.into();
        fs::create_dir_all(root.join("packs"))?;
        fs::create_dir_all(root.join("downloads"))?;
        fs::create_dir_all(root.join("staging"))?;
        Ok(PackStore { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn pack_dir(&self, id: &str) -> PathBuf {
        self.root.join("packs").join(id)
    }

    pub(crate) fn download_dir(&self, id: &str) -> PathBuf {
        self.root.join("downloads").join(id)
    }

    /// Шлях до файлу встановленого пакета.
    pub fn file_path(&self, id: &str, rel: &str) -> Option<PathBuf> {
        if !is_safe_rel_path(rel) {
            return None;
        }
        let p = self.pack_dir(id).join(rel);
        p.is_file().then_some(p)
    }

    /// Усі встановлені пакети (пошкоджені маніфести пропускаються).
    pub fn installed(&self) -> Vec<PackManifest> {
        let mut out = Vec::new();
        let Ok(rd) = fs::read_dir(self.root.join("packs")) else {
            return out;
        };
        for e in rd.flatten() {
            let m = e.path().join("manifest.json");
            if let Ok(s) = fs::read_to_string(&m) {
                if let Ok(man) = PackManifest::from_json(&s) {
                    out.push(man);
                }
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    pub fn get(&self, id: &str) -> Option<PackManifest> {
        let s = fs::read_to_string(self.pack_dir(id).join("manifest.json")).ok()?;
        PackManifest::from_json(&s).ok()
    }

    pub fn remove(&self, id: &str) -> Result<(), Error> {
        if !crate::pack::is_safe_id(id) {
            return Err(Error::Invalid("id".into()));
        }
        for d in [self.pack_dir(id), self.download_dir(id)] {
            if d.exists() {
                fs::remove_dir_all(d)?;
            }
        }
        Ok(())
    }

    /// Вільне місце не рахуємо тут (це робить `hardware`), лише байти пакетів.
    pub fn used_bytes(&self) -> u64 {
        self.installed().iter().map(|m| m.size_bytes()).sum()
    }

    /// Імпорт із теки (USB-флешка, SD-картка, спільна тека ПК):
    /// у теці має бути `manifest.json` і всі файли з нього.
    pub fn install_from_dir(
        &self,
        src: &Path,
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<PackManifest, Error> {
        let man = PackManifest::from_json(&fs::read_to_string(src.join("manifest.json"))?)?;
        let staging = self.fresh_staging(&man.id)?;
        let total = man.size_bytes();
        let mut done = 0u64;
        for f in &man.files {
            let from = src.join(&f.path);
            let to = staging.join(&f.path);
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            copy_verified(&from, &to, f, &mut |n| {
                done += n;
                progress(done, total)
            })?;
        }
        self.commit(&man, &staging)?;
        Ok(man)
    }

    /// Імпорт з одного файлу `.uapack` (звичайний tar без стиснення:
    /// ZIM, PMTiles і моделі вже стиснуті). Першим має йти `manifest.json`.
    pub fn install_from_archive(
        &self,
        archive: &Path,
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<PackManifest, Error> {
        let file = fs::File::open(archive)?;
        let mut tar = tar::Archive::new(file);
        let mut entries = tar.entries()?;
        let first = entries
            .next()
            .ok_or_else(|| Error::Invalid("порожній архів".into()))??;
        if first.path()?.to_string_lossy().trim_start_matches("./") != "manifest.json" {
            return Err(Error::Invalid(
                "першим у .uapack має бути manifest.json".into(),
            ));
        }
        let mut s = String::new();
        { first }.read_to_string(&mut s)?;
        let man = PackManifest::from_json(&s)?;
        let staging = self.fresh_staging(&man.id)?;
        let total = man.size_bytes();
        let mut done = 0u64;
        let mut seen = Vec::new();
        for entry in entries {
            let mut entry = entry?;
            if !entry.header().entry_type().is_file() {
                continue;
            }
            let path = entry
                .path()?
                .to_string_lossy()
                .trim_start_matches("./")
                .to_string();
            let Some(f) = man.files.iter().find(|f| f.path == path) else {
                continue; // зайвий файл ігноруємо
            };
            let to = staging.join(&f.path);
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            write_verified(&mut entry, &to, f, &mut |n| {
                done += n;
                progress(done, total)
            })?;
            seen.push(f.path.clone());
        }
        if let Some(missing) = man.files.iter().find(|f| !seen.contains(&f.path)) {
            let _ = fs::remove_dir_all(&staging);
            return Err(Error::Invalid(format!(
                "в архіві бракує файлу {}",
                missing.path
            )));
        }
        self.commit(&man, &staging)?;
        Ok(man)
    }

    /// Пакує встановлений пакет у `.uapack` (щоб передати на флешку чи іншому).
    pub fn export_archive(&self, id: &str, out: &Path) -> Result<(), Error> {
        let man = self
            .get(id)
            .ok_or_else(|| Error::Invalid(format!("пакет {id} не встановлено")))?;
        let file = fs::File::create(out)?;
        let mut b = tar::Builder::new(file);
        let json = man.to_json();
        let mut h = tar::Header::new_gnu();
        h.set_size(json.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        b.append_data(&mut h, "manifest.json", json.as_bytes())?;
        for f in &man.files {
            b.append_path_with_name(self.pack_dir(id).join(&f.path), &f.path)?;
        }
        b.into_inner()?.flush()?;
        Ok(())
    }

    pub(crate) fn fresh_staging(&self, id: &str) -> Result<PathBuf, Error> {
        let d = self.root.join("staging").join(id);
        if d.exists() {
            fs::remove_dir_all(&d)?;
        }
        fs::create_dir_all(&d)?;
        Ok(d)
    }

    /// Атомарно (наскільки дозволяє ФС) замінює встановлену версію.
    pub(crate) fn commit(&self, man: &PackManifest, staging: &Path) -> Result<(), Error> {
        // Адреси завантаження потрібні лише каталогу.
        let mut man = man.clone();
        for f in &mut man.files {
            f.url = None;
        }
        let man = &man;
        fs::write(staging.join("manifest.json"), man.to_json())?;
        let dest = self.pack_dir(&man.id);
        let old = self.root.join("staging").join(format!("{}.old", man.id));
        if old.exists() {
            fs::remove_dir_all(&old)?;
        }
        if dest.exists() {
            fs::rename(&dest, &old)?;
        }
        fs::rename(staging, &dest)?;
        if old.exists() {
            let _ = fs::remove_dir_all(&old);
        }
        Ok(())
    }
}

/// SHA-256 файлу у hex.
pub fn sha256_file(path: &Path) -> Result<String, Error> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

fn copy_verified(
    from: &Path,
    to: &Path,
    f: &PackFile,
    progress: &mut dyn FnMut(u64),
) -> Result<(), Error> {
    let mut src =
        fs::File::open(from).map_err(|e| Error::Invalid(format!("не знайдено {}: {e}", f.path)))?;
    write_verified(&mut src, to, f, progress)
}

fn write_verified(
    src: &mut dyn Read,
    to: &Path,
    f: &PackFile,
    progress: &mut dyn FnMut(u64),
) -> Result<(), Error> {
    let mut out = fs::File::create(to)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut size = 0u64;
    loop {
        let n = src.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        out.write_all(&buf[..n])?;
        size += n as u64;
        progress(n as u64);
    }
    out.flush()?;
    let got = hex::encode(h.finalize());
    if size != f.size || !got.eq_ignore_ascii_case(&f.sha256) {
        let _ = fs::remove_file(to);
        return Err(Error::Checksum(f.path.clone()));
    }
    Ok(())
}

/// Допоміжне для інструментів і тестів: маніфест для теки з файлами.
pub fn manifest_for_dir(dir: &Path, mut base: PackManifest) -> Result<PackManifest, Error> {
    let mut files = Vec::new();
    walk(dir, dir, &mut files)?;
    files.sort_by(|a: &PackFile, b| a.path.cmp(&b.path));
    base.files = files;
    base.validate()?;
    Ok(base)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<PackFile>) -> Result<(), Error> {
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            walk(root, &p, out)?;
        } else {
            let rel = p
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if rel == "manifest.json" {
                continue;
            }
            out.push(PackFile {
                url: None,
                size: fs::metadata(&p)?.len(),
                sha256: sha256_file(&p)?,
                path: rel,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::pack::{PackKind, PackTier};

    pub fn sample_pack(dir: &Path, id: &str) -> PackManifest {
        fs::create_dir_all(dir.join("articles")).unwrap();
        fs::write(
            dir.join("cards.json"),
            r#"[{"id":"x1","category":"household","title":"Свічка","steps":["Не лишайте без нагляду"],"source":"тест"}]"#,
        )
        .unwrap();
        fs::write(dir.join("articles/gen.md"), "# Генератор\nТільки надворі.").unwrap();
        let m = manifest_for_dir(
            dir,
            PackManifest {
                id: id.into(),
                tier: PackTier::Pocket,
                kind: PackKind::Content,
                title: "Тест".into(),
                description: String::new(),
                version: "1".into(),
                files: vec![],
                content_license: "CC0-1.0".into(),
                region: None,
                model_id: None,
            },
        )
        .unwrap();
        fs::write(dir.join("manifest.json"), m.to_json()).unwrap();
        m
    }

    #[test]
    fn install_dir_archive_roundtrip() {
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("usb/pocket");
        sample_pack(&src, "pocket");
        let store = PackStore::open(t.path().join("a")).unwrap();
        let mut last = (0, 0);
        let m = store
            .install_from_dir(&src, &mut |d, tot| last = (d, tot))
            .unwrap();
        assert_eq!(last.0, last.1);
        assert_eq!(store.installed().len(), 1);
        assert!(store.file_path("pocket", "articles/gen.md").is_some());
        assert!(store.file_path("pocket", "../x").is_none());

        let arch = t.path().join("p.uapack");
        store.export_archive("pocket", &arch).unwrap();
        let store2 = PackStore::open(t.path().join("b")).unwrap();
        let m2 = store2.install_from_archive(&arch, &mut |_, _| {}).unwrap();
        assert_eq!(m, m2);
        // Повторне встановлення замінює версію.
        store2.install_from_archive(&arch, &mut |_, _| {}).unwrap();
        assert_eq!(store2.installed().len(), 1);
        store2.remove("pocket").unwrap();
        assert!(store2.installed().is_empty());
    }

    #[test]
    fn rejects_tampered_files() {
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("p");
        sample_pack(&src, "pocket");
        fs::write(src.join("cards.json"), "[]").unwrap();
        let store = PackStore::open(t.path().join("a")).unwrap();
        let err = store.install_from_dir(&src, &mut |_, _| {}).unwrap_err();
        assert!(matches!(err, Error::Checksum(_)));
        assert!(store.installed().is_empty());
    }
}
