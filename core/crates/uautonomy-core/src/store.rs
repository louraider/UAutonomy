//! Встановлені пакети на диску.
//!
//! ```text
//! <root>/packs/<id>/manifest.json     встановлений пакет
//! <root>/packs/<id>/<файли>
//! <root>/downloads/<id>/<файли>.part  незавершені завантаження (докачуються)
//! <root>/staging/                     тимчасові теки під час імпорту
//! <root>/staging/<id>~old             попередня версія під час заміни
//! ```
//!
//! Встановлюються лише пакети з дійсним підписом проєкту ([`crate::sign`]).
//! Заміна версії відбувається двома перейменуваннями тек; якщо живлення
//! зникне посередині, [`PackStore::open`] поверне попередню версію.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest, Sha256};

use crate::pack::{is_safe_id, is_safe_rel_path, PackFile, PackManifest};
use crate::sign::Trust;
use crate::Error;

#[derive(Debug, Clone)]
pub struct PackStore {
    root: PathBuf,
    trust: Arc<Trust>,
}

/// Тека попередньої версії під час заміни. `~` не буває в id пакета.
const OLD_SUFFIX: &str = "~old";

/// Кроки заміни версії (для тестів зі штучним «вимкненням живлення»).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Step {
    ManifestWritten,
    OldMovedAside,
    NewInPlace,
    Done,
}

impl PackStore {
    /// Відкриває сховище з ключами, вшитими в застосунок.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, Error> {
        Self::open_with_trust(root, Trust::builtin())
    }

    /// Відкриває сховище з іншим набором довірених ключів (тести, хаб).
    pub fn open_with_trust(root: impl Into<PathBuf>, trust: Trust) -> Result<Self, Error> {
        let root = root.into();
        fs::create_dir_all(root.join("packs"))?;
        fs::create_dir_all(root.join("downloads"))?;
        fs::create_dir_all(root.join("staging"))?;
        let store = PackStore {
            root,
            trust: Arc::new(trust),
        };
        store.recover()?;
        Ok(store)
    }

    pub fn trust(&self) -> &Trust {
        &self.trust
    }

    /// Доводить до кінця перервану заміну версії: якщо нова версія вже на
    /// місці й читається, прибирає стару; інакше повертає стару.
    fn recover(&self) -> Result<(), Error> {
        for e in fs::read_dir(self.root.join("staging"))?.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(id) = name.strip_suffix(OLD_SUFFIX) else {
                continue;
            };
            if !is_safe_id(id) || !e.path().is_dir() {
                continue;
            }
            self.recover_one(id)?;
        }
        Ok(())
    }

    fn recover_one(&self, id: &str) -> Result<(), Error> {
        let old = self.old_dir(id);
        if !old.exists() {
            return Ok(());
        }
        let dest = self.pack_dir(id);
        if read_manifest(&dest).is_some() {
            fs::remove_dir_all(&old)?;
        } else {
            if dest.exists() {
                fs::remove_dir_all(&dest)?;
            }
            fs::rename(&old, &dest)?;
            sync_dir(&self.root.join("packs"));
        }
        Ok(())
    }

    fn old_dir(&self, id: &str) -> PathBuf {
        self.root.join("staging").join(format!("{id}{OLD_SUFFIX}"))
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

    /// Усі маніфести на диску, і довірені, і ні (пошкоджені пропускаються).
    fn all_manifests(&self) -> Vec<PackManifest> {
        let mut out = Vec::new();
        let Ok(rd) = fs::read_dir(self.root.join("packs")) else {
            return out;
        };
        for e in rd.flatten() {
            if let Some(man) = read_manifest(&e.path()) {
                out.push(man);
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Встановлені пакети з дійсним підписом. Лише їх застосунок читає,
    /// показує й роздає іншим.
    pub fn installed(&self) -> Vec<PackManifest> {
        self.all_manifests()
            .into_iter()
            .filter(|m| self.trust.verify_pack(m).is_ok())
            .collect()
    }

    /// Пакети без дійсного підпису (наприклад, із тестових збірок до
    /// появи підписів). Не використовуються; їх варто видалити.
    pub fn untrusted(&self) -> Vec<PackManifest> {
        self.all_manifests()
            .into_iter()
            .filter(|m| self.trust.verify_pack(m).is_err())
            .collect()
    }

    /// Встановлений пакет із дійсним підписом.
    pub fn get(&self, id: &str) -> Option<PackManifest> {
        read_manifest(&self.pack_dir(id)).filter(|m| self.trust.verify_pack(m).is_ok())
    }

    /// Чи можна ставити цей пакет: підпис дійсний, і це не старіша
    /// редакція за вже встановлену (захист від відкату виправлень).
    pub fn check_new(&self, man: &PackManifest) -> Result<(), Error> {
        man.validate()?;
        self.trust.verify_pack(man)?;
        if let Some(cur) = self.get(&man.id) {
            if cur.issued() > man.issued() {
                return Err(Error::Untrusted(format!(
                    "«{}» версії {} старіший за встановлений ({}); щоб повернути стару редакцію, спершу видаліть пакет",
                    man.title, man.version, cur.version
                )));
            }
        }
        Ok(())
    }

    pub fn remove(&self, id: &str) -> Result<(), Error> {
        if !crate::pack::is_safe_id(id) {
            return Err(Error::Invalid("id".into()));
        }
        for d in [self.pack_dir(id), self.download_dir(id), self.old_dir(id)] {
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
        self.check_new(&man)?;
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
        self.check_new(&man)?;
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

    /// Замінює встановлену версію новою з теки `staging`.
    ///
    /// Порядок: маніфест записується й скидається на диск; стара версія
    /// відсувається в `staging/<id>~old`; нова тека перейменовується на
    /// місце; стара видаляється. Перейменування теки атомарне, тож після
    /// збою на диску завжди є або стара, або нова повна версія, а
    /// [`PackStore::open`] доводить заміну до кінця.
    pub(crate) fn commit(&self, man: &PackManifest, staging: &Path) -> Result<(), Error> {
        self.commit_until(man, staging, Step::Done)
    }

    fn commit_until(&self, man: &PackManifest, staging: &Path, stop: Step) -> Result<(), Error> {
        self.check_new(man)?;
        // Адреси завантаження потрібні лише каталогу.
        let mut man = man.clone();
        for f in &mut man.files {
            f.url = None;
        }
        let man = &man;
        // Залишки попереднього збою спершу доводимо до кінця.
        self.recover_one(&man.id)?;
        let mut mf = fs::File::create(staging.join("manifest.json"))?;
        mf.write_all(man.to_json().as_bytes())?;
        mf.sync_all()?;
        drop(mf);
        sync_dir(staging);
        if stop == Step::ManifestWritten {
            return Ok(());
        }
        let dest = self.pack_dir(&man.id);
        let old = self.old_dir(&man.id);
        let had_old = dest.exists();
        if had_old {
            // На Windows не вдасться, якщо файл пакета відкритий; тоді
            // нічого не змінено і встановлена версія працює далі.
            fs::rename(&dest, &old)?;
            sync_dir(&self.root.join("staging"));
        }
        if stop == Step::OldMovedAside {
            return Ok(());
        }
        if let Err(e) = fs::rename(staging, &dest) {
            if had_old {
                let _ = fs::rename(&old, &dest);
            }
            return Err(e.into());
        }
        sync_dir(&self.root.join("packs"));
        if stop == Step::NewInPlace {
            return Ok(());
        }
        if had_old {
            let _ = fs::remove_dir_all(&old);
        }
        Ok(())
    }
}

fn read_manifest(dir: &Path) -> Option<PackManifest> {
    let s = fs::read_to_string(dir.join("manifest.json")).ok()?;
    PackManifest::from_json(&s).ok()
}

/// Скидає на диск запис про перейменування в теці (лише Unix; на Windows
/// теку так відкрити не можна, там NTFS журналює метадані сама).
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(d) = fs::File::open(dir) {
        let _ = d.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
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
    out.sync_all()?;
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

    /// Сховище, що довіряє лише тестовому ключу.
    pub fn test_store(root: &Path) -> PackStore {
        PackStore::open_with_trust(root, crate::sign::tests::test_trust()).unwrap()
    }

    /// Підписує маніфест у теці пакета тестовим ключем.
    pub fn sign_dir(dir: &Path, issued: u64) -> PackManifest {
        let mut m =
            PackManifest::from_json(&fs::read_to_string(dir.join("manifest.json")).unwrap())
                .unwrap();
        crate::sign::sign_pack(&mut m, &crate::sign::tests::test_key(), issued);
        fs::write(dir.join("manifest.json"), m.to_json()).unwrap();
        m
    }

    /// Підписаний тестовий пакет у теці `dir`.
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
                signature: None,
            },
        )
        .unwrap();
        fs::write(dir.join("manifest.json"), m.to_json()).unwrap();
        sign_dir(dir, 1000)
    }

    #[test]
    fn install_dir_archive_roundtrip() {
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("usb/pocket");
        sample_pack(&src, "pocket");
        let store = test_store(&t.path().join("a"));
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
        let store2 = test_store(&t.path().join("b"));
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
        let store = test_store(&t.path().join("a"));
        let err = store.install_from_dir(&src, &mut |_, _| {}).unwrap_err();
        assert!(matches!(err, Error::Checksum(_)));
        assert!(store.installed().is_empty());
    }

    #[test]
    fn rejects_unsigned_unknown_key_and_older_edition() {
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("p");
        sample_pack(&src, "pocket");
        let store = test_store(&t.path().join("a"));

        // Без підпису (як пакет з флешки, зібраний будь-ким).
        let mut m =
            PackManifest::from_json(&fs::read_to_string(src.join("manifest.json")).unwrap())
                .unwrap();
        m.signature = None;
        fs::write(src.join("manifest.json"), m.to_json()).unwrap();
        let err = store.install_from_dir(&src, &mut |_, _| {}).unwrap_err();
        assert!(matches!(err, Error::Untrusted(_)), "{err}");

        // Підписаний чужим ключем.
        let other = ed25519_dalek::SigningKey::from_bytes(&[3u8; 32]);
        crate::sign::sign_pack(&mut m, &other, 1000);
        fs::write(src.join("manifest.json"), m.to_json()).unwrap();
        assert!(matches!(
            store.install_from_dir(&src, &mut |_, _| {}),
            Err(Error::Untrusted(_))
        ));

        // Підмінений вміст разом із переписаним SHA-256 у маніфесті.
        fs::write(src.join("cards.json"), r#"[{"id":"x1","category":"household","title":"Шкідлива порада","steps":["…"],"source":"?"}]"#).unwrap();
        let mut forged = m.clone();
        let f = forged
            .files
            .iter_mut()
            .find(|f| f.path == "cards.json")
            .unwrap();
        f.sha256 = sha256_file(&src.join("cards.json")).unwrap();
        f.size = fs::metadata(src.join("cards.json")).unwrap().len();
        forged.signature = sample_pack(&t.path().join("q"), "pocket").signature;
        fs::write(src.join("manifest.json"), forged.to_json()).unwrap();
        assert!(matches!(
            store.install_from_dir(&src, &mut |_, _| {}),
            Err(Error::Untrusted(_))
        ));
        assert!(store.installed().is_empty());

        // Новіша редакція ставиться, старіша після неї — ні.
        sample_pack(&src, "pocket");
        sign_dir(&src, 2000);
        store.install_from_dir(&src, &mut |_, _| {}).unwrap();
        sign_dir(&src, 1500);
        let err = store.install_from_dir(&src, &mut |_, _| {}).unwrap_err();
        assert!(err.to_string().contains("старіший"), "{err}");
        assert_eq!(store.get("pocket").unwrap().issued(), 2000);
        // Після видалення стару редакцію можна поставити свідомо.
        store.remove("pocket").unwrap();
        store.install_from_dir(&src, &mut |_, _| {}).unwrap();
    }

    #[test]
    fn untrusted_installed_packs_are_hidden() {
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("p");
        sample_pack(&src, "pocket");
        let store = test_store(&t.path().join("a"));
        store.install_from_dir(&src, &mut |_, _| {}).unwrap();
        // Те саме сховище без довірених ключів (як стара непідписана установка).
        let bare = PackStore::open_with_trust(t.path().join("a"), Trust::default()).unwrap();
        assert!(bare.installed().is_empty());
        assert!(bare.get("pocket").is_none());
        assert_eq!(bare.untrusted().len(), 1);
        bare.remove("pocket").unwrap();
        assert!(store.untrusted().is_empty() && store.installed().is_empty());
    }

    /// «Вимкнення живлення» на кожному кроці заміни версії: після
    /// повторного відкриття завжди є повна стара або нова версія.
    #[test]
    fn update_survives_crash_at_every_step() {
        for stop in [Step::ManifestWritten, Step::OldMovedAside, Step::NewInPlace] {
            let t = tempfile::tempdir().unwrap();
            let src = t.path().join("p");
            sample_pack(&src, "pocket");
            let root = t.path().join("a");
            let store = test_store(&root);
            store.install_from_dir(&src, &mut |_, _| {}).unwrap();

            // Нова редакція: інший вміст картки, новіший підпис.
            fs::write(src.join("cards.json"), r#"[{"id":"x1","category":"household","title":"Свічка v2","steps":["Нове"],"source":"тест"}]"#).unwrap();
            let base =
                PackManifest::from_json(&fs::read_to_string(src.join("manifest.json")).unwrap())
                    .unwrap();
            let mut v2 = manifest_for_dir(
                &src,
                PackManifest {
                    signature: None,
                    ..base
                },
            )
            .unwrap();
            crate::sign::sign_pack(&mut v2, &crate::sign::tests::test_key(), 2000);
            let staging = store.fresh_staging("pocket").unwrap();
            for f in &v2.files {
                let to = staging.join(&f.path);
                fs::create_dir_all(to.parent().unwrap()).unwrap();
                fs::copy(src.join(&f.path), to).unwrap();
            }
            store.commit_until(&v2, &staging, stop).unwrap();

            let reopened = test_store(&root);
            let got = reopened.get("pocket").expect("пакет не зник");
            let want = if stop >= Step::NewInPlace { 2000 } else { 1000 };
            assert_eq!(got.issued(), want, "{stop:?}");
            let cards =
                fs::read_to_string(reopened.file_path("pocket", "cards.json").unwrap()).unwrap();
            assert_eq!(cards.contains("v2"), want == 2000, "{stop:?}");
            assert!(
                !root.join("staging").join("pocket~old").exists(),
                "{stop:?}"
            );
            // Після відновлення оновлення проходить звичайним шляхом.
            let staging = reopened.fresh_staging("pocket").unwrap();
            for f in &v2.files {
                let to = staging.join(&f.path);
                fs::create_dir_all(to.parent().unwrap()).unwrap();
                fs::copy(src.join(&f.path), to).unwrap();
            }
            reopened.commit(&v2, &staging).unwrap();
            assert_eq!(reopened.get("pocket").unwrap().issued(), 2000);
        }
    }
}
