//! Завантаження пакетів з докачкою (HTTP Range) і перевіркою SHA-256.
//! Джерело — інтернет-сервер, домашній хаб або інший телефон у Wi-Fi.

use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::pack::{Catalog, PackManifest};
use crate::store::{sha256_file, PackStore};
use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .build()
        .into()
}

/// Завантажує каталог пакетів.
pub fn fetch_catalog(url: &str) -> Result<Catalog, Error> {
    let mut resp = agent().get(url).call().map_err(net)?;
    let body = resp.body_mut().read_to_string().map_err(net)?;
    let mut c = Catalog::from_json(&body)?;
    if c.base_url.is_empty() {
        c.base_url = url
            .rsplit_once('/')
            .map(|(b, _)| b.to_string())
            .unwrap_or_default();
    }
    Ok(c)
}

fn net(e: ureq::Error) -> Error {
    Error::Network(e.to_string())
}

/// Завантажує й встановлює пакет. Перерване завантаження продовжується з
/// того ж місця при наступному виклику. `cancel` можна виставити з іншого
/// потоку, щоб зупинити (докачка збережеться).
pub fn download_pack(
    store: &PackStore,
    catalog: &Catalog,
    pack: &PackManifest,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<PackManifest, Error> {
    pack.validate()?;
    let dl = store.download_dir(&pack.id);
    fs::create_dir_all(&dl)?;
    let total = pack.size_bytes();
    let mut done_before = 0u64;
    let agent = agent();
    for f in &pack.files {
        let final_path = dl.join(&f.path);
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if final_path.is_file()
            && fs::metadata(&final_path)?.len() == f.size
            && sha256_file(&final_path)?.eq_ignore_ascii_case(&f.sha256)
        {
            done_before += f.size;
            progress(Progress {
                done: done_before,
                total,
            });
            continue;
        }
        let part = final_path.with_file_name(format!(
            "{}.part",
            final_path.file_name().unwrap().to_string_lossy()
        ));
        let url = catalog.file_url(pack, f);
        let mut attempts = 0;
        loop {
            attempts += 1;
            match fetch_resumable(&agent, &url, &part, f.size, cancel, &mut |n| {
                progress(Progress {
                    done: done_before + n,
                    total,
                })
            }) {
                Ok(()) => break,
                Err(Error::Cancelled) => return Err(Error::Cancelled),
                Err(e) if attempts >= 4 => return Err(e),
                Err(_) => std::thread::sleep(Duration::from_secs(2 * attempts)),
            }
        }
        if !sha256_file(&part)?.eq_ignore_ascii_case(&f.sha256) {
            fs::remove_file(&part)?;
            return Err(Error::Checksum(f.path.clone()));
        }
        fs::rename(&part, &final_path)?;
        done_before += f.size;
    }
    store.commit(pack, &dl)?;
    Ok(pack.clone())
}

fn fetch_resumable(
    agent: &ureq::Agent,
    url: &str,
    part: &Path,
    size: u64,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Result<(), Error> {
    let mut have = fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    if have > size {
        fs::remove_file(part)?;
        have = 0;
    }
    if have == size {
        return Ok(());
    }
    let mut req = agent.get(url);
    if have > 0 {
        req = req.header("Range", format!("bytes={have}-"));
    }
    let mut resp = req.call().map_err(net)?;
    let mut out = if resp.status() == 206 {
        fs::OpenOptions::new().append(true).open(part)?
    } else {
        have = 0;
        fs::File::create(part)?
    };
    progress(have);
    let mut reader = resp.body_mut().with_config().limit(size + 1).reader();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        if cancel.load(Ordering::Relaxed) {
            out.flush()?;
            return Err(Error::Cancelled);
        }
        let n = reader
            .read(&mut buf)
            .map_err(|e| Error::Network(e.to_string()))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
        have += n as u64;
        progress(have);
    }
    out.flush()?;
    if have != size {
        return Err(Error::Network(format!("отримано {have} з {size} байтів")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::share::{ShareServer, ShareState};
    use std::sync::{Arc, RwLock};

    #[test]
    fn downloads_from_share_server_and_resumes() {
        let t = tempfile::tempdir().unwrap();
        // Телефон А: має пакет.
        let src = t.path().join("src");
        crate::store::tests::sample_pack(&src, "pocket");
        let a = PackStore::open(t.path().join("a")).unwrap();
        a.install_from_dir(&src, &mut |_, _| {}).unwrap();
        let state = Arc::new(RwLock::new(ShareState::new(a, crate::Library::default())));
        let server = ShareServer::start(state, 0).unwrap();
        let base = format!("http://127.0.0.1:{}", server.port());

        // Телефон Б: частково завантажений файл має докачатися.
        let b = PackStore::open(t.path().join("b")).unwrap();
        let cat = fetch_catalog(&format!("{base}/packs/catalog.json")).unwrap();
        let pack = cat.packs[0].clone();
        let dl = b.download_dir("pocket");
        fs::create_dir_all(&dl).unwrap();
        let cards = fs::read(src.join("cards.json")).unwrap();
        fs::write(dl.join("cards.json.part"), &cards[..10]).unwrap();

        let cancel = AtomicBool::new(false);
        let mut last = Progress { done: 0, total: 0 };
        download_pack(&b, &cat, &pack, &cancel, &mut |p| last = p).unwrap();
        assert_eq!(last.done, last.total);
        assert_eq!(b.installed(), vec![pack.clone()]);
        assert_eq!(
            fs::read(b.file_path("pocket", "cards.json").unwrap()).unwrap(),
            cards
        );

        cancel.store(true, Ordering::Relaxed);
        b.remove("pocket").unwrap();
        assert!(matches!(
            download_pack(&b, &cat, &pack, &cancel, &mut |_| {}),
            Err(Error::Cancelled)
        ));
        server.stop();
    }
}
