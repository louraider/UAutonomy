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

/// Завантажує каталог і його підпис `<url>.sig`. Каталог із інтернету
/// (https) без дійсного підпису відхиляється. Каталог з іншого телефона чи
/// хаба в локальній мережі (http) складається на ходу і не підписаний, але
/// кожен пакет у ньому однаково має підпис проєкту, який перевіряється
/// перед встановленням.
pub fn fetch_catalog_checked(url: &str, trust: &crate::sign::Trust) -> Result<Catalog, Error> {
    let agent = agent();
    let mut resp = agent.get(url).call().map_err(net)?;
    let body = resp.body_mut().read_to_vec().map_err(net)?;
    if url.starts_with("https://") {
        let sig_url = format!("{url}.sig");
        let mut sresp = agent
            .get(&sig_url)
            .call()
            .map_err(|e| Error::Untrusted(format!("немає підпису каталогу ({sig_url}): {e}")))?;
        let sig_text = sresp.body_mut().read_to_string().map_err(net)?;
        let sig: crate::sign::DetachedSignature = serde_json::from_str(&sig_text)
            .map_err(|_| Error::Untrusted("пошкоджений підпис каталогу".into()))?;
        trust.verify_bytes(&body, &sig)?;
    }
    let text = String::from_utf8(body).map_err(|_| Error::Invalid("каталог не UTF-8".into()))?;
    let mut c = Catalog::from_json(&text)?;
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
    // Підпис перевіряємо до завантаження, щоб не качати гігабайти даремно,
    // і ще раз під час встановлення.
    store.check_new(pack)?;
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
    out.sync_all()?;
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
        let a = crate::store::tests::test_store(&t.path().join("a"));
        a.install_from_dir(&src, &mut |_, _| {}).unwrap();
        let state = Arc::new(RwLock::new(ShareState::new(a, crate::Library::default())));
        let server = ShareServer::start(state, 0).unwrap();
        let base = format!("http://127.0.0.1:{}", server.port());

        // Телефон Б: частково завантажений файл має докачатися.
        let b = crate::store::tests::test_store(&t.path().join("b"));
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

        // Як у GitHub Releases: файл за прямою адресою з перенаправленням
        // (302) на інший сервер, докачка має пережити перенаправлення.
        let redirect = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let rport = redirect.server_addr().to_ip().unwrap().port();
        let target = base.clone();
        let rthread = std::thread::spawn(move || {
            for req in redirect.incoming_requests().take(1) {
                let loc = format!("{target}/packs/pocket/cards.json");
                let h = tiny_http::Header::from_bytes(&b"Location"[..], loc.as_bytes()).unwrap();
                let _ = req.respond(tiny_http::Response::empty(302).with_header(h));
            }
        });
        let mut cat2 = cat.clone();
        let mut pack2 = pack.clone();
        pack2.files.retain(|f| f.path == "cards.json");
        pack2.files[0].url = Some(format!(
            "http://127.0.0.1:{rport}/release/pocket--cards.json"
        ));
        crate::sign::sign_pack(&mut pack2, &crate::sign::tests::test_key(), 1000);
        cat2.packs = vec![pack2.clone()];
        let c = crate::store::tests::test_store(&t.path().join("c"));
        let dl = c.download_dir("pocket");
        fs::create_dir_all(&dl).unwrap();
        fs::write(dl.join("cards.json.part"), &cards[..10]).unwrap();
        download_pack(&c, &cat2, &pack2, &cancel, &mut |_| {}).unwrap();
        assert_eq!(
            fs::read(c.file_path("pocket", "cards.json").unwrap()).unwrap(),
            cards
        );
        assert!(
            c.get("pocket").unwrap().files[0].url.is_none(),
            "адреса не зберігається у встановленому маніфесті"
        );
        rthread.join().unwrap();

        // Каталог із локальної мережі без підпису читається (пакети в ньому
        // підписані окремо); підмінений пакет у ньому не встановлюється.
        let trust = crate::sign::tests::test_trust();
        let cat3 = fetch_catalog_checked(&format!("{base}/packs/catalog.json"), &trust).unwrap();
        let mut evil = cat3.packs[0].clone();
        evil.title = "Підмінений".into();
        let d = crate::store::tests::test_store(&t.path().join("d"));
        assert!(matches!(
            download_pack(&d, &cat3, &evil, &cancel, &mut |_| {}),
            Err(Error::Untrusted(_))
        ));
        assert!(!d.download_dir("pocket").exists(), "нічого не завантажено");

        cancel.store(true, Ordering::Relaxed);
        b.remove("pocket").unwrap();
        assert!(matches!(
            download_pack(&b, &cat, &pack, &cancel, &mut |_| {}),
            Err(Error::Cancelled)
        ));
        server.stop();
    }
}
