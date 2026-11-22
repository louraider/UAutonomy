use std::path::Path;

use uautonomy_core::models::{self, ModelFormat};
use uautonomy_core::{hardware, PackKind};

use crate::state;

pub struct HardwareView {
    pub total_ram_mb: u64,
    pub available_ram_mb: u64,
    pub cpu_cores: u32,
    pub cpu_brand: String,
    pub os: String,
    pub arch: String,
    pub free_disk_mb: u64,
    pub is_mobile: bool,
}

pub struct RecommendationView {
    pub model_id: Option<String>,
    pub ai_by_default: bool,
    pub runnable_models: Vec<String>,
    pub pack_tier: String,
    pub reason: String,
}

pub struct ModelView {
    pub id: String,
    pub title: String,
    pub family: String,
    pub format: String,
    pub size_bytes: u64,
    pub min_ram_mb: u64,
    pub context_size: u32,
    pub license: String,
    pub pack_id: Option<String>,
    pub notes: String,
}

/// Модель, файл якої вже є на пристрої.
pub struct LocalModel {
    pub id: String,
    pub title: String,
    pub path: String,
    pub format: String,
    pub size_bytes: u64,
    pub context_size: u32,
}

fn data_dir() -> std::path::PathBuf {
    state::app_ref()
        .map(|a| a.data_dir.clone())
        .unwrap_or_else(|_| ".".into())
}

pub fn hardware_info() -> HardwareView {
    let h = hardware::detect(&data_dir());
    HardwareView {
        total_ram_mb: h.total_ram_mb,
        available_ram_mb: h.available_ram_mb,
        cpu_cores: h.cpu_cores,
        cpu_brand: h.cpu_brand,
        os: h.os,
        arch: h.arch,
        free_disk_mb: h.free_disk_mb,
        is_mobile: h.is_mobile,
    }
}

pub fn recommendation() -> RecommendationView {
    let h = hardware::detect(&data_dir());
    let r = hardware::recommend(&h, &models::default_catalog());
    RecommendationView {
        model_id: r.model_id,
        ai_by_default: r.ai_by_default,
        runnable_models: r.runnable_models,
        pack_tier: serde_json::to_value(r.pack_tier)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default(),
        reason: r.reason,
    }
}

#[flutter_rust_bridge::frb(sync)]
pub fn model_catalog() -> Vec<ModelView> {
    models::default_catalog()
        .into_iter()
        .map(|m| ModelView {
            format: fmt(m.format).into(),
            id: m.id,
            title: m.title,
            family: m.family,
            size_bytes: m.size_bytes,
            min_ram_mb: m.min_ram_mb,
            context_size: m.context_size,
            license: m.license,
            pack_id: m.pack_id,
            notes: m.notes,
        })
        .collect()
}

fn fmt(f: ModelFormat) -> &'static str {
    match f {
        ModelFormat::Gguf => "gguf",
        ModelFormat::Litertlm => "litertlm",
    }
}

/// Моделі з пакетів (kind = model) і файли, додані вручну в `<дані>/models/`.
#[flutter_rust_bridge::frb(sync)]
pub fn local_models() -> Vec<LocalModel> {
    let Ok(a) = state::app_ref() else {
        return Vec::new();
    };
    let catalog = models::default_catalog();
    let mut out = Vec::new();
    let store = a.shared.read().unwrap().store.clone();
    for m in store
        .installed()
        .into_iter()
        .filter(|m| m.kind == PackKind::Model)
    {
        for f in &m.files {
            let Some(format) = ModelFormat::from_file_name(&f.path) else {
                continue;
            };
            let spec = m
                .model_id
                .as_ref()
                .and_then(|id| catalog.iter().find(|c| &c.id == id));
            out.push(LocalModel {
                id: m.model_id.clone().unwrap_or_else(|| m.id.clone()),
                title: spec
                    .map(|s| s.title.clone())
                    .unwrap_or_else(|| m.title.clone()),
                path: store
                    .pack_dir(&m.id)
                    .join(&f.path)
                    .to_string_lossy()
                    .into_owned(),
                format: fmt(format).into(),
                size_bytes: f.size,
                context_size: spec.map(|s| s.context_size).unwrap_or(4096),
            });
        }
    }
    if let Ok(rd) = std::fs::read_dir(a.data_dir.join("models")) {
        for e in rd.flatten() {
            let p = e.path();
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if let Some(format) = ModelFormat::from_file_name(&name) {
                out.push(LocalModel {
                    id: format!("custom:{name}"),
                    title: format!("{name} (власна)"),
                    path: p.to_string_lossy().into_owned(),
                    format: fmt(format).into(),
                    size_bytes: e.metadata().map(|m| m.len()).unwrap_or(0),
                    context_size: 4096,
                });
            }
        }
    }
    out
}

/// Копіює файл моделі (GGUF / LiteRT-LM) у теку застосунку. Так можна
/// випробувати MamayLM, Lapa чи власну донавчену модель без оновлення.
pub fn import_model_file(path: String) -> anyhow::Result<LocalModel> {
    let a = state::app_ref()?;
    let src = Path::new(&path);
    let name = src
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| anyhow::anyhow!("немає імені файлу"))?;
    let format = ModelFormat::from_file_name(&name)
        .ok_or_else(|| anyhow::anyhow!("потрібен файл .gguf або .litertlm"))?;
    let dir = a.data_dir.join("models");
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(&name);
    if src != dest {
        std::fs::copy(src, &dest)?;
    }
    Ok(LocalModel {
        id: format!("custom:{name}"),
        title: format!("{name} (власна)"),
        path: dest.to_string_lossy().into_owned(),
        format: fmt(format).into(),
        size_bytes: std::fs::metadata(&dest)?.len(),
        context_size: 4096,
    })
}

pub fn remove_model_file(path: String) -> anyhow::Result<()> {
    let a = state::app_ref()?;
    let p = Path::new(&path);
    if p.parent() != Some(a.data_dir.join("models").as_path()) {
        anyhow::bail!("видаляти можна лише власні моделі");
    }
    std::fs::remove_file(p)?;
    Ok(())
}

pub struct RamStatus {
    pub total_mb: u64,
    pub available_mb: u64,
}

/// Поточна пам'ять (для діагностики ШІ: скільки з'їла модель).
pub fn ram_status() -> RamStatus {
    let h = hardware::detect(&data_dir());
    RamStatus {
        total_mb: h.total_ram_mb,
        available_mb: h.available_ram_mb,
    }
}
