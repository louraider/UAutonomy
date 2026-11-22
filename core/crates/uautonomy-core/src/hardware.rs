//! Визначення можливостей пристрою і вибір моделі та пакета.

use std::path::Path;

use serde::Serialize;

use crate::models::ModelSpec;
use crate::pack::PackTier;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HardwareInfo {
    pub total_ram_mb: u64,
    pub available_ram_mb: u64,
    pub cpu_cores: u32,
    pub cpu_brand: String,
    pub os: String,
    pub arch: String,
    /// Вільне місце на диску, де лежать пакети.
    pub free_disk_mb: u64,
    /// Телефон чи ПК (визначає, які моделі пропонувати).
    pub is_mobile: bool,
}

/// Збирає відомості про пристрій. `data_dir` — тека з пакетами.
pub fn detect(data_dir: &Path) -> HardwareInfo {
    use sysinfo::{Disks, System};
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_list(sysinfo::CpuRefreshKind::nothing());
    let cpu_brand = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .unwrap_or_default();
    let disks = Disks::new_with_refreshed_list();
    let canon = data_dir
        .canonicalize()
        .unwrap_or_else(|_| data_dir.to_path_buf());
    let free_disk_mb = disks
        .list()
        .iter()
        .filter(|d| canon.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| d.available_space() / 1_048_576)
        .unwrap_or(0);
    HardwareInfo {
        total_ram_mb: sys.total_memory() / 1_048_576,
        available_ram_mb: sys.available_memory() / 1_048_576,
        cpu_cores: sys.cpus().len().max(1) as u32,
        cpu_brand,
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        free_disk_mb,
        is_mobile: matches!(std::env::consts::OS, "android" | "ios"),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Recommendation {
    /// Найкраща модель для пристрою (None — ШІ не тягне).
    pub model_id: Option<String>,
    /// Чи вмикати ШІ без запиту (інакше лише за бажанням користувача).
    pub ai_by_default: bool,
    /// Моделі, які взагалі можна запустити, від найлегшої.
    pub runnable_models: Vec<String>,
    pub pack_tier: PackTier,
    /// Пояснення українською для екрана налаштувань.
    pub reason: String,
}

/// Чиста функція вибору, щоб її можна було тестувати.
pub fn recommend(hw: &HardwareInfo, models: &[ModelSpec]) -> Recommendation {
    let ram = hw.total_ram_mb;
    // Модель має вміститися на диск з запасом 1 ГБ під пакети.
    let fits_disk =
        |m: &ModelSpec| hw.free_disk_mb == 0 || m.size_bytes / 1_048_576 + 1024 < hw.free_disk_mb;
    let mut runnable: Vec<&ModelSpec> = models
        .iter()
        .filter(|m| ram >= m.min_ram_mb && fits_disk(m))
        .filter(|m| !(hw.is_mobile && m.min_ram_mb > 12_000))
        .collect();
    runnable.sort_by_key(|m| m.min_ram_mb);
    // Найсильніша з рекомендованих для цього обсягу ОЗП.
    let best = runnable
        .iter()
        .filter(|m| ram >= m.recommended_ram_mb)
        .max_by_key(|m| m.recommended_ram_mb)
        .or_else(|| {
            runnable
                .iter()
                .filter(|m| m.recommended_ram_mb < 1_000_000)
                .min_by_key(|m| m.min_ram_mb)
        });
    let ai_by_default = best.is_some_and(|m| ram >= m.recommended_ram_mb);

    let disk_gb = hw.free_disk_mb / 1024;
    let pack_tier = if hw.free_disk_mb == 0 {
        PackTier::Pocket
    } else if !hw.is_mobile && disk_gb >= 120 {
        PackTier::Community
    } else if disk_gb >= 30 {
        PackTier::Family
    } else if disk_gb >= 2 {
        PackTier::Pocket
    } else {
        PackTier::Base
    };

    let ram_gb = format!("{:.1}", ram as f64 / 1024.0);
    let reason = match best {
        None => format!(
            "На пристрої {ram_gb} ГБ ОЗП: цього замало для локального ШІ. Бібліотека, пошук і карти працюють повністю."
        ),
        Some(m) if ai_by_default => format!("{ram_gb} ГБ ОЗП: рекомендуємо {} (≈{:.1} ГБ).", m.title, m.size_bytes as f64 / 1e9),
        Some(m) => format!(
            "{ram_gb} ГБ ОЗП: {} запуститься, але може працювати повільно. Вмикайте за бажанням.",
            m.title
        ),
    };
    Recommendation {
        model_id: best.map(|m| m.id.clone()),
        ai_by_default,
        runnable_models: runnable.iter().map(|m| m.id.clone()).collect(),
        pack_tier,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::default_catalog;

    fn hw(ram_gb: u64, disk_gb: u64, mobile: bool) -> HardwareInfo {
        HardwareInfo {
            total_ram_mb: ram_gb * 1024,
            available_ram_mb: ram_gb * 512,
            cpu_cores: 8,
            cpu_brand: String::new(),
            os: if mobile { "android" } else { "windows" }.into(),
            arch: "x86_64".into(),
            free_disk_mb: disk_gb * 1024,
            is_mobile: mobile,
        }
    }

    #[test]
    fn budget_phone_has_no_ai() {
        let r = recommend(&hw(3, 16, true), &default_catalog());
        assert_eq!(r.model_id, None);
        assert!(!r.ai_by_default);
        assert_eq!(r.pack_tier, PackTier::Pocket);
    }

    #[test]
    fn mid_phone_gets_optional_e2b() {
        let r = recommend(&hw(4, 64, true), &default_catalog());
        assert_eq!(r.model_id.as_deref(), Some("gemma-4-e2b-it-q4"));
        assert!(!r.ai_by_default);
        let r = recommend(&hw(8, 64, true), &default_catalog());
        assert_eq!(r.model_id.as_deref(), Some("gemma-4-e2b-it-q4"));
        assert!(r.ai_by_default);
        assert_eq!(r.pack_tier, PackTier::Family);
    }

    #[test]
    fn pcs_scale_up() {
        let r = recommend(&hw(16, 500, false), &default_catalog());
        assert_eq!(r.model_id.as_deref(), Some("gemma-4-e4b-it-q4"));
        assert_eq!(r.pack_tier, PackTier::Community);
        let r = recommend(&hw(64, 500, false), &default_catalog());
        assert_eq!(r.model_id.as_deref(), Some("gemma-4-26b-a4b-it-q4"));
        assert!(r
            .runnable_models
            .contains(&"mamaylm-gemma-3-4b-q4".to_string()));
        // Мало місця на диску: велика модель не пропонується.
        let r = recommend(&hw(64, 8, false), &default_catalog());
        assert_eq!(r.model_id.as_deref(), Some("gemma-4-e4b-it-q4"));
    }

    #[test]
    fn detect_runs() {
        let h = detect(Path::new("."));
        assert!(h.total_ram_mb > 0);
        assert!(h.cpu_cores >= 1);
    }
}
