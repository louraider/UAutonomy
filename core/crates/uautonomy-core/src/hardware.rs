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
#[cfg(not(target_os = "ios"))]
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
        .filter(|d| path_on_mount(&canon, d.mount_point()))
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

/// iOS: sysinfo тут не збирається, тож читаємо системні виклики напряму.
/// Доступна пам'ять — `os_proc_available_memory`: скільки ще може взяти
/// саме цей застосунок до примусового завершення (з entitlement
/// increased-memory-limit це більше).
#[cfg(target_os = "ios")]
pub fn detect(data_dir: &Path) -> HardwareInfo {
    extern "C" {
        fn os_proc_available_memory() -> libc::size_t;
    }
    fn sysctl_u64(name: &str) -> u64 {
        let c = std::ffi::CString::new(name).expect("назва sysctl");
        let mut v: u64 = 0;
        let mut len = std::mem::size_of::<u64>();
        // SAFETY: буфер на 8 байтів і його довжина передані правильно.
        let rc = unsafe {
            libc::sysctlbyname(
                c.as_ptr(),
                (&mut v as *mut u64).cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc == 0 {
            v
        } else {
            0
        }
    }
    fn sysctl_string(name: &str) -> String {
        let c = std::ffi::CString::new(name).expect("назва sysctl");
        let mut buf = [0u8; 64];
        let mut len = buf.len();
        // SAFETY: буфер і його довжина передані правильно.
        let rc = unsafe {
            libc::sysctlbyname(
                c.as_ptr(),
                buf.as_mut_ptr().cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc != 0 {
            return String::new();
        }
        let end = buf[..len].iter().position(|&b| b == 0).unwrap_or(len);
        String::from_utf8_lossy(&buf[..end]).into_owned()
    }
    fn free_disk(path: &Path) -> u64 {
        use std::os::unix::ffi::OsStrExt;
        let Ok(c) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
            return 0;
        };
        // SAFETY: statfs заповнює структуру, яку ми надали.
        let mut st: libc::statfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statfs(c.as_ptr(), &mut st) } != 0 {
            return 0;
        }
        st.f_bavail * u64::from(st.f_bsize)
    }
    // SAFETY: функція без аргументів з libSystem (iOS 13+).
    let available = unsafe { os_proc_available_memory() } as u64;
    let cores = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1);
    HardwareInfo {
        total_ram_mb: sysctl_u64("hw.memsize") / 1_048_576,
        available_ram_mb: available / 1_048_576,
        cpu_cores: cores,
        cpu_brand: sysctl_string("hw.machine"),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        free_disk_mb: free_disk(data_dir) / 1_048_576,
        is_mobile: true,
    }
}

/// Чи лежить `path` на диску з точкою монтування `mount`. На Windows
/// `canonicalize` дає `\\?\C:\...`, а диск — `C:\`, і регістр літер
/// довільний; без нормалізації вільне місце завжди було 0.
#[cfg_attr(target_os = "ios", allow(dead_code))]
fn path_on_mount(path: &Path, mount: &Path) -> bool {
    fn norm(p: &Path) -> String {
        let s = p.to_string_lossy().replace('/', "\\");
        let s = s
            .strip_prefix("\\\\?\\UNC\\")
            .map(|r| format!("\\\\{r}"))
            .unwrap_or_else(|| s.strip_prefix("\\\\?\\").unwrap_or(&s).to_string());
        s.to_lowercase()
    }
    if cfg!(windows) || mount.to_string_lossy().contains(':') {
        let (p, m) = (norm(path), norm(mount));
        let m = m.trim_end_matches('\\');
        p == m || p.starts_with(&format!("{m}\\"))
    } else {
        path.starts_with(mount)
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
        .filter(|m| !(hw.is_mobile && (m.desktop_only || m.min_ram_mb > 12_000)))
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
        // ПК з 16+ ГБ: основна модель MamayLM 12B, ШІ вмикається сам.
        let r = recommend(&hw(16, 500, false), &default_catalog());
        assert_eq!(r.model_id.as_deref(), Some("mamaylm-gemma-3-12b-it-q4"));
        assert!(r.ai_by_default);
        assert_eq!(r.pack_tier, PackTier::Community);
        // 26B більше не радимо навіть на 64 ГБ, але запустити її можна.
        let r = recommend(&hw(64, 500, false), &default_catalog());
        assert_eq!(r.model_id.as_deref(), Some("mamaylm-gemma-3-12b-it-q4"));
        assert!(r
            .runnable_models
            .contains(&"gemma-4-26b-a4b-it-q4".to_string()));
        // Слабкий ПК (8–15 ГБ): Gemma 4 E4B.
        for gb in [8, 12, 15] {
            let r = recommend(&hw(gb, 500, false), &default_catalog());
            assert_eq!(r.model_id.as_deref(), Some("gemma-4-e4b-it-q4"), "{gb} ГБ");
            assert!(r.ai_by_default);
        }
        // Мало місця на диску: MamayLM (7,3 ГБ) не влазить, лишається E4B.
        let r = recommend(&hw(64, 6, false), &default_catalog());
        assert_eq!(r.model_id.as_deref(), Some("gemma-4-e4b-it-q4"));
    }

    #[test]
    fn pc_falls_back_when_mamaylm_missing() {
        // Каталог без MamayLM (ще не опубліковано): ПК бере E4B, а без неї E2B.
        let mut c = default_catalog();
        c.retain(|m| m.family != "mamaylm");
        let r = recommend(&hw(32, 500, false), &c);
        assert_eq!(r.model_id.as_deref(), Some("gemma-4-e4b-it-q4"));
        c.retain(|m| m.id == "gemma-4-e2b-it-q4");
        let r = recommend(&hw(32, 500, false), &c);
        assert_eq!(r.model_id.as_deref(), Some("gemma-4-e2b-it-q4"));
    }

    #[test]
    fn phones_stay_on_e2b() {
        for gb in [8, 12, 16, 24] {
            let r = recommend(&hw(gb, 256, true), &default_catalog());
            assert_eq!(r.model_id.as_deref(), Some("gemma-4-e2b-it-q4"), "{gb} ГБ");
            assert_eq!(r.runnable_models, vec!["gemma-4-e2b-it-q4".to_string()]);
        }
    }

    #[test]
    fn detect_runs() {
        let h = detect(Path::new("."));
        assert!(h.total_ram_mb > 0);
        assert!(h.cpu_cores >= 1);
    }

    #[test]
    fn windows_paths_match_their_drive() {
        let p = Path::new(r"\\?\C:\Users\I\AppData\Roaming\org.uautonomy\app");
        assert!(path_on_mount(p, Path::new(r"C:\")));
        assert!(path_on_mount(Path::new(r"c:\data"), Path::new(r"C:\")));
        assert!(!path_on_mount(p, Path::new(r"D:\")));
        assert!(path_on_mount(Path::new("/home/u/data"), Path::new("/")));
    }
}
