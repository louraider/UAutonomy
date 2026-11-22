//! Headless-хаб «Автономії» для Raspberry Pi, домашніх серверів і ПК, а також
//! інструменти для збирання пакетів.
//!
//! ```text
//! uautonomy-hub serve   --data <тека> [--port 8642] [--app <файл>]...
//! uautonomy-hub install --data <тека> <тека-пакета | файл.uapack>
//! uautonomy-hub pack    <тека> --id <id> --title <назва> [--tier pocket] [--kind content]
//!                       [--version 0.1.0] [--license CC-BY-4.0] [--region kyiv] [--model <id>]
//! uautonomy-hub export  <тека-пакета> <файл.uapack>
//! uautonomy-hub catalog <тека-з-пакетами> [--base-url https://...]
//!                       [--release-url https://github.com/.../releases/download/<tag> --stage <тека>]
//!                       [--merge зовнішні-пакети.json]...   (додає лише відсутні id)
//! uautonomy-hub regions                            (області для карт, JSON)
//! uautonomy-hub zim-check <файл.zim> [запит]...      (перевірка читача на справжньому ZIM)
//!
//! `--release-url` / `--stage`: для плаского хостингу (GitHub Releases) кожен
//! файл копіюється в `--stage` під іменем `<id>--<шлях>` і отримує пряму адресу.
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, RwLock};

use uautonomy_core::share::{ShareServer, ShareState, DEFAULT_PORT};
use uautonomy_core::{content, store, Catalog, PackKind, PackManifest, PackStore, PackTier};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("serve") => serve(&args[1..]),
        Some("install") => install(&args[1..]),
        Some("pack") => pack(&args[1..]),
        Some("export") => export(&args[1..]),
        Some("catalog") => catalog(&args[1..]),
        Some("zim-check") => zim_check(&args[1..]),
        Some("regions") => {
            println!(
                "{}",
                serde_json::to_string_pretty(uautonomy_core::regions::REGIONS).unwrap()
            );
            Ok(())
        }
        _ => {
            eprintln!(
                "uautonomy-hub {}\nКоманди: serve, install, pack, export, catalog, regions, zim-check (див. документацію в main.rs)",
                uautonomy_core::version()
            );
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Помилка: {e}");
            ExitCode::FAILURE
        }
    }
}

type R = Result<(), Box<dyn std::error::Error>>;

fn opt(args: &[String], name: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == name).map(|w| w[1].clone())
}

fn opts(args: &[String], name: &str) -> Vec<String> {
    args.windows(2)
        .filter(|w| w[0] == name)
        .map(|w| w[1].clone())
        .collect()
}

fn positional(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut skip = false;
    for a in args {
        if skip {
            skip = false;
        } else if a.starts_with("--") {
            skip = true;
        } else {
            out.push(a.clone());
        }
    }
    out
}

fn serve(args: &[String]) -> R {
    let data = opt(args, "--data").ok_or("потрібно --data <тека>")?;
    let port: u16 = opt(args, "--port")
        .map(|p| p.parse())
        .transpose()?
        .unwrap_or(DEFAULT_PORT);
    let store = PackStore::open(&data)?;
    let mut lib = content::builtin_base();
    for w in content::load_installed(&mut lib, &store) {
        eprintln!("Попередження: {w}");
    }
    let mut state = ShareState::new(store, lib);
    state.app_files = opts(args, "--app").into_iter().map(PathBuf::from).collect();
    state.device_name = opt(args, "--name").unwrap_or_else(|| "Хаб".into());
    let server = ShareServer::start(Arc::new(RwLock::new(state)), port)?;
    println!("Автономія-хаб працює. Відкрийте на телефоні:");
    for u in server.urls() {
        println!("  {u}");
    }
    loop {
        std::thread::park();
    }
}

fn install(args: &[String]) -> R {
    let data = opt(args, "--data").ok_or("потрібно --data <тека>")?;
    let store = PackStore::open(&data)?;
    for src in positional(args) {
        let p = Path::new(&src);
        let mut report =
            |d: u64, t: u64| eprint!("\r{:>3}%", (d * 100).checked_div(t).unwrap_or(100));
        let m = if p.is_dir() {
            store.install_from_dir(p, &mut report)?
        } else {
            store.install_from_archive(p, &mut report)?
        };
        eprintln!("\rВстановлено: {} ({})", m.title, m.id);
    }
    Ok(())
}

fn parse_enum<T: serde::de::DeserializeOwned>(v: &str) -> Result<T, Box<dyn std::error::Error>> {
    Ok(serde_json::from_value(serde_json::Value::String(
        v.to_string(),
    ))?)
}

fn pack(args: &[String]) -> R {
    let dir = positional(args)
        .into_iter()
        .next()
        .ok_or("потрібна тека пакета")?;
    let base = PackManifest {
        id: opt(args, "--id").ok_or("потрібно --id")?,
        tier: parse_enum::<PackTier>(&opt(args, "--tier").unwrap_or("pocket".into()))?,
        kind: parse_enum::<PackKind>(&opt(args, "--kind").unwrap_or("content".into()))?,
        title: opt(args, "--title").ok_or("потрібно --title")?,
        description: opt(args, "--description").unwrap_or_default(),
        version: opt(args, "--version").unwrap_or("0.1.0".into()),
        files: Vec::new(),
        content_license: opt(args, "--license").unwrap_or("CC-BY-4.0".into()),
        region: opt(args, "--region"),
        model_id: opt(args, "--model"),
    };
    let m = store::manifest_for_dir(Path::new(&dir), base)?;
    std::fs::write(Path::new(&dir).join("manifest.json"), m.to_json())?;
    println!(
        "manifest.json: {} файлів, {:.1} МБ",
        m.files.len(),
        m.size_bytes() as f64 / 1_048_576.0
    );
    Ok(())
}

fn export(args: &[String]) -> R {
    let p = positional(args);
    let (dir, out) = (
        p.first().ok_or("потрібна тека")?,
        p.get(1).ok_or("потрібен вихідний файл")?,
    );
    // Встановлюємо в тимчасове сховище, щоб перевірити SHA-256, і пакуємо.
    let tmp = std::env::temp_dir().join(format!("uautonomy-export-{}", std::process::id()));
    let store = PackStore::open(&tmp)?;
    let m = store.install_from_dir(Path::new(dir), &mut |_, _| {})?;
    store.export_archive(&m.id, Path::new(out))?;
    let _ = std::fs::remove_dir_all(&tmp);
    println!("{out}: {}", m.title);
    Ok(())
}

fn catalog(args: &[String]) -> R {
    let dir = positional(args)
        .into_iter()
        .next()
        .ok_or("потрібна тека з пакетами")?;
    let mut packs = Vec::new();
    let mut folders = std::collections::HashMap::new();
    for e in std::fs::read_dir(&dir)? {
        let folder = e?.path();
        let m = folder.join("manifest.json");
        if m.is_file() {
            let man = PackManifest::from_json(&std::fs::read_to_string(m)?)?;
            folders.insert(man.id.clone(), folder);
            packs.push(man);
        }
    }
    if let Some(release) = opt(args, "--release-url") {
        let stage =
            PathBuf::from(opt(args, "--stage").ok_or("з --release-url потрібно --stage <тека>")?);
        std::fs::create_dir_all(&stage)?;
        for p in &mut packs {
            for f in &mut p.files {
                let flat = format!("{}--{}", p.id, f.path.replace('/', "--"));
                let src = folders[&p.id].join(&f.path);
                let dst = stage.join(&flat);
                if std::fs::hard_link(&src, &dst).is_err() {
                    std::fs::copy(&src, &dst)?;
                }
                f.url = Some(format!("{}/{}", release.trim_end_matches('/'), flat));
            }
        }
    }
    for extra in opts(args, "--merge") {
        let text = std::fs::read_to_string(&extra)?;
        let more: Vec<PackManifest> = match serde_json::from_str::<Catalog>(&text) {
            Ok(c) => c.packs,
            Err(_) => serde_json::from_str(&text)?,
        };
        // Нові пакети з теки мають перевагу; з --merge додаються лише ті,
        // яких ще немає (так попередній каталог не затирає свіжі збірки).
        for m in more {
            m.validate()?;
            if !packs.iter().any(|p| p.id == m.id) {
                packs.push(m);
            }
        }
    }
    packs.sort_by(|a, b| a.id.cmp(&b.id));
    let c = Catalog {
        base_url: opt(args, "--base-url").unwrap_or_default(),
        packs,
    };
    let out = Path::new(&dir).join("catalog.json");
    std::fs::write(&out, serde_json::to_string_pretty(&c)?)?;
    println!("{}: {} пакетів", out.display(), c.packs.len());
    Ok(())
}

/// Відкриває справжній ZIM (наприклад, з Kiwix) нашим читачем: метадані,
/// головна сторінка, пошук за назвою й уривки для ШІ. Помилка — ненульовий код.
fn zim_check(args: &[String]) -> R {
    use uautonomy_core::zim::Zim;
    let pos = positional(args);
    let path = pos.first().ok_or("потрібен файл .zim")?;
    let started = std::time::Instant::now();
    let zim = Arc::new(Zim::open(path)?);
    let meta = |k: &str| zim.metadata(k).unwrap_or_default();
    println!(
        "{path}: {} записів, простір імен «{}», «{}», мова {}, дата {} ({} мс)",
        zim.entry_count(),
        zim.content_namespace(),
        meta("Title"),
        meta("Language"),
        meta("Date"),
        started.elapsed().as_millis()
    );
    let main = zim.main_page().ok_or("немає головної сторінки")?;
    let main = zim.resolve(&main)?;
    let (mime, data) = zim.content(&main)?;
    println!("порядок заголовків: {}", zim.title_index_info());
    println!("головна: {} ({mime}, {} Б)", main.path(), data.len());
    if data.is_empty() {
        return Err("порожня головна сторінка".into());
    }
    let mut lib = uautonomy_core::Library::new(Vec::new());
    lib.add_zim("check", zim.clone());
    for q in &pos[1..] {
        let t = std::time::Instant::now();
        let hits = lib.search(q, 5);
        let passages = lib.retrieve(q, 3);
        println!(
            "«{q}»: {} результатів, {} уривків ({} мс)",
            hits.len(),
            passages.len(),
            t.elapsed().as_millis()
        );
        for h in hits.iter().take(3) {
            println!("  - {}", h.title);
        }
        if let Some(p) = passages.first() {
            let preview: String = p.text.chars().take(160).collect();
            println!("  уривок: {preview}");
        }
    }
    Ok(())
}
