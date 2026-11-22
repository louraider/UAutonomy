use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use uautonomy_core::{download, PackKind, PackManifest, PackTier};

use crate::frb_generated::StreamSink;
use crate::state;

static CANCEL: AtomicBool = AtomicBool::new(false);

pub struct PackView {
    pub id: String,
    pub title: String,
    pub description: String,
    pub tier: String,
    pub kind: String,
    pub version: String,
    pub size_bytes: u64,
    pub installed: bool,
    pub region: Option<String>,
    pub model_id: Option<String>,
}

pub struct ProgressView {
    pub done: u64,
    pub total: u64,
}

fn tier_str(t: PackTier) -> &'static str {
    match t {
        PackTier::Base => "base",
        PackTier::Pocket => "pocket",
        PackTier::Family => "family",
        PackTier::Community => "community",
    }
}

fn kind_str(k: PackKind) -> &'static str {
    match k {
        PackKind::Content => "content",
        PackKind::Map => "map",
        PackKind::Model => "model",
    }
}

fn view(m: &PackManifest, installed: bool) -> PackView {
    PackView {
        id: m.id.clone(),
        title: m.title.clone(),
        description: m.description.clone(),
        tier: tier_str(m.tier).into(),
        kind: kind_str(m.kind).into(),
        version: m.version.clone(),
        size_bytes: m.size_bytes(),
        installed,
        region: m.region.clone(),
        model_id: m.model_id.clone(),
    }
}

#[flutter_rust_bridge::frb(sync)]
pub fn installed_packs() -> Vec<PackView> {
    let Ok(a) = state::app_ref() else {
        return Vec::new();
    };
    let s = a.shared.read().unwrap();
    s.store.installed().iter().map(|m| view(m, true)).collect()
}

/// Каталог з інтернету, хаба чи іншого телефона. Приймає адресу
/// `catalog.json` або просто адресу пристрою (`http://192.168.43.1:8642/`).
pub fn fetch_catalog(url: String) -> anyhow::Result<Vec<PackView>> {
    let a = state::app_ref()?;
    let url = url.trim().to_string();
    let url = if url.ends_with(".json") {
        url
    } else {
        format!("{}/packs/catalog.json", url.trim_end_matches('/'))
    };
    let url = if url.contains("://") {
        url
    } else {
        format!("http://{url}")
    };
    let cat = download::fetch_catalog(&url)?;
    let installed = a.shared.read().unwrap().store.installed();
    let out = cat
        .packs
        .iter()
        .map(|m| {
            view(
                m,
                installed
                    .iter()
                    .any(|i| i.id == m.id && i.version == m.version),
            )
        })
        .collect();
    *a.catalog.lock().unwrap() = Some(cat);
    Ok(out)
}

/// Завантажує пакет з останнього отриманого каталогу. Перерване
/// завантаження продовжиться з місця зупинки.
pub fn download_pack(id: String, sink: StreamSink<ProgressView>) -> anyhow::Result<PackView> {
    let a = state::app_ref()?;
    let cat = a
        .catalog
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| anyhow::anyhow!("спершу відкрийте каталог"))?;
    let pack = cat
        .packs
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("пакета {id} немає в каталозі"))?;
    let need = pack.size_bytes();
    let hw = uautonomy_core::hardware::detect(&a.data_dir);
    if hw.free_disk_mb > 0 && need / 1_048_576 + 200 > hw.free_disk_mb {
        anyhow::bail!(
            "замало місця: потрібно {:.1} ГБ, вільно {:.1} ГБ",
            need as f64 / 1e9,
            hw.free_disk_mb as f64 / 1024.0
        );
    }
    CANCEL.store(false, Ordering::Relaxed);
    let store = a.shared.read().unwrap().store.clone();
    let mut last = 0u64;
    let m = download::download_pack(&store, &cat, &pack, &CANCEL, &mut |p| {
        // Не засипаємо UI: оновлення кожні ~256 КБ.
        if p.done == p.total || p.done - last.min(p.done) > 256 * 1024 {
            last = p.done;
            let _ = sink.add(ProgressView {
                done: p.done,
                total: p.total,
            });
        }
    })?;
    state::reload_library(a);
    Ok(view(&m, true))
}

#[flutter_rust_bridge::frb(sync)]
pub fn cancel_download() {
    CANCEL.store(true, Ordering::Relaxed);
}

/// Імпорт з флешки чи файлу: тека з `manifest.json` або файл `.uapack`.
pub fn import_pack(path: String, sink: StreamSink<ProgressView>) -> anyhow::Result<PackView> {
    let a = state::app_ref()?;
    let store = a.shared.read().unwrap().store.clone();
    let p = Path::new(&path);
    let mut report = |done: u64, total: u64| {
        let _ = sink.add(ProgressView { done, total });
    };
    let m = if p.is_dir() {
        store.install_from_dir(p, &mut report)?
    } else if p.file_name().is_some_and(|n| n == "manifest.json") {
        store.install_from_dir(p.parent().unwrap_or(p), &mut report)?
    } else {
        store.install_from_archive(p, &mut report)?
    };
    state::reload_library(a);
    Ok(view(&m, true))
}

/// Зберігає пакет одним файлом `.uapack` (щоб передати флешкою).
pub fn export_pack(id: String, out_path: String) -> anyhow::Result<()> {
    let a = state::app_ref()?;
    let store = a.shared.read().unwrap().store.clone();
    store.export_archive(&id, Path::new(&out_path))?;
    Ok(())
}

pub fn remove_pack(id: String) -> anyhow::Result<()> {
    let a = state::app_ref()?;
    let store = a.shared.read().unwrap().store.clone();
    store.remove(&id)?;
    state::reload_library(a);
    Ok(())
}
