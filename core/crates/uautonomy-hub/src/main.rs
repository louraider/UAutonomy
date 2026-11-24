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
//!                       [--sign-key-env PACK_SIGNING_KEY]  (підписати пакети й каталог)
//! uautonomy-hub keygen  <файл-приватного-ключа>            (новий ключ; друкує публічний)
//! uautonomy-hub sign    <тека-пакета> --key-env PACK_SIGNING_KEY
//! uautonomy-hub verify  <тека-пакета | catalog.json>     (перевірка вбудованими ключами)
//! uautonomy-hub regions                            (області для карт, JSON)
//! uautonomy-hub zim-check <файл.zim> [запит]...      (перевірка читача на справжньому ZIM)
//! uautonomy-hub rag [--data <тека>] [--context-tokens 4096] < питання.jsonl
//!                       (маршрут, промпт і джерела так само, як у застосунку; для eval)
//!
//! `--release-url` / `--stage`: для плаского хостингу (GitHub Releases) кожен
//! файл копіюється в `--stage` під іменем `<id>--<шлях>` і отримує пряму адресу.
//!
//! Підписи: приватний ключ — 32 байти в hex у змінній середовища (секрет CI),
//! ніколи не в репозиторії. Хаб довіряє ключам із `assets/keys/trusted.json`
//! і додатково з `UAUTONOMY_TRUSTED_KEYS` (публічні ключі hex через кому),
//! щоб власник хаба міг роздавати й власні підписані пакети.
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, RwLock};

use uautonomy_core::share::{ShareServer, ShareState, DEFAULT_PORT};
use uautonomy_core::sign::{self, Trust};
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
        Some("rag") => rag(&args[1..]),
        Some("keygen") => keygen(&args[1..]),
        Some("sign") => sign_cmd(&args[1..]),
        Some("verify") => verify(&args[1..]),
        Some("regions") => {
            println!(
                "{}",
                serde_json::to_string_pretty(uautonomy_core::regions::REGIONS).unwrap()
            );
            Ok(())
        }
        _ => {
            eprintln!(
                "uautonomy-hub {}\nКоманди: serve, install, pack, export, catalog, keygen, sign, verify, regions, zim-check, rag (див. документацію в main.rs)",
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

/// Вбудовані ключі плюс `UAUTONOMY_TRUSTED_KEYS`.
fn trust() -> Result<Trust, Box<dyn std::error::Error>> {
    let mut t = Trust::builtin();
    if let Ok(extra) = std::env::var("UAUTONOMY_TRUSTED_KEYS") {
        for k in extra.split(',').filter(|k| !k.trim().is_empty()) {
            t.add_hex(k)?;
        }
    }
    Ok(t)
}

fn open_store(dir: impl Into<PathBuf>) -> Result<PackStore, Box<dyn std::error::Error>> {
    Ok(PackStore::open_with_trust(dir, trust()?)?)
}

fn signing_key(env: &str) -> Result<ed25519_dalek::SigningKey, Box<dyn std::error::Error>> {
    let v = std::env::var(env).map_err(|_| format!("змінна {env} з приватним ключем не задана"))?;
    Ok(sign::signing_key_from_hex(&v)?)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Новий ключ підпису. Приватний ключ пишеться у файл (не на екран),
/// публічний друкується: його додають у `assets/keys/trusted.json`.
fn keygen(args: &[String]) -> R {
    let out = positional(args)
        .into_iter()
        .next()
        .ok_or("потрібен файл для приватного ключа")?;
    let out = Path::new(&out);
    if out.exists() {
        return Err(format!("{} вже існує, не перезаписую", out.display()).into());
    }
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| format!("getrandom: {e}"))?;
    let key = ed25519_dalek::SigningKey::from_bytes(&seed);
    {
        use std::io::Write;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(out)?;
        writeln!(f, "{}", hex::encode(seed))?;
    }
    println!("{}", sign::public_hex(&key));
    eprintln!(
        "Приватний ключ записано в {} (id {}). Нікому не показуйте і не комітьте.",
        out.display(),
        sign::key_id(&key.verifying_key())
    );
    Ok(())
}

fn sign_cmd(args: &[String]) -> R {
    let env = opt(args, "--key-env").unwrap_or("PACK_SIGNING_KEY".into());
    let key = signing_key(&env)?;
    for dir in positional(args) {
        let path = Path::new(&dir).join("manifest.json");
        let mut m = PackManifest::from_json(&std::fs::read_to_string(&path)?)?;
        sign::sign_pack(&mut m, &key, now());
        std::fs::write(&path, m.to_json())?;
        println!(
            "{}: підписано ({})",
            m.id,
            m.signature.as_ref().unwrap().key_id
        );
    }
    Ok(())
}

fn verify(args: &[String]) -> R {
    let t = trust()?;
    if t.is_empty() {
        return Err("немає довірених ключів (assets/keys/trusted.json порожній)".into());
    }
    for p in positional(args) {
        let p = Path::new(&p);
        if p.is_dir() {
            let m = PackManifest::from_json(&std::fs::read_to_string(p.join("manifest.json"))?)?;
            t.verify_pack(&m)?;
            println!("{}: підпис дійсний", m.id);
        } else {
            let bytes = std::fs::read(p)?;
            let sig_path = PathBuf::from(format!("{}.sig", p.display()));
            let sig: sign::DetachedSignature =
                serde_json::from_str(&std::fs::read_to_string(&sig_path)?)?;
            t.verify_bytes(&bytes, &sig)?;
            let c = Catalog::from_json(std::str::from_utf8(&bytes)?)?;
            for m in &c.packs {
                t.verify_pack(m)?;
            }
            println!(
                "{}: каталог і {} пакетів підписані",
                p.display(),
                c.packs.len()
            );
        }
    }
    Ok(())
}

fn serve(args: &[String]) -> R {
    let data = opt(args, "--data").ok_or("потрібно --data <тека>")?;
    let port: u16 = opt(args, "--port")
        .map(|p| p.parse())
        .transpose()?
        .unwrap_or(DEFAULT_PORT);
    let store = open_store(&data)?;
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
    let store = open_store(&data)?;
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
        signature: None,
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
    let store = open_store(&tmp)?;
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
    let key = match opt(args, "--sign-key-env") {
        Some(env) => Some(signing_key(&env)?),
        None => None,
    };
    if let Some(key) = &key {
        // Пакети, вже підписані цим ключем (з попереднього каталогу),
        // зберігають дату підпису; решту підписуємо зараз.
        let own = Trust::from_keys(&[key.verifying_key()]);
        let issued = now();
        for p in &mut packs {
            if own.verify_pack(p).is_err() {
                sign::sign_pack(p, key, issued);
            }
        }
    }
    let c = Catalog {
        base_url: opt(args, "--base-url").unwrap_or_default(),
        packs,
    };
    let out = Path::new(&dir).join("catalog.json");
    let json = serde_json::to_string_pretty(&c)?;
    std::fs::write(&out, &json)?;
    if let Some(key) = &key {
        let sig = sign::sign_bytes(json.as_bytes(), key);
        std::fs::write(
            Path::new(&dir).join("catalog.json.sig"),
            serde_json::to_string(&sig)?,
        )?;
        println!("{}: {} пакетів, підписано", out.display(), c.packs.len());
    } else {
        println!("{}: {} пакетів (без підпису)", out.display(), c.packs.len());
    }
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
    println!(
        "порядок заголовків: {}; HTML-статей: {}",
        zim.title_index_info(),
        zim.articles().count()
    );
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

/// Для eval: читає JSONL з полями `id` і `question` і для кожного питання
/// друкує JSON-рядок з тим самим маршрутом, промптом і джерелами, що й
/// застосунок (`rag::route` + `rag::build_prompt`). Так eval міряє продукт.
fn rag(args: &[String]) -> R {
    use std::io::BufRead;
    use uautonomy_core::rag;
    let mut lib = content::builtin_base();
    if let Some(dir) = opt(args, "--data") {
        let store = open_store(&dir)?;
        for w in content::load_installed(&mut lib, &store) {
            eprintln!("попередження: {w}");
        }
    }
    let ctx_tokens: usize = opt(args, "--context-tokens")
        .map(|v| v.parse())
        .transpose()?
        .unwrap_or(4096);
    // Як у застосунку (app/rust/src/api/ai.rs): половина контексту, ~3 символи на токен.
    let max_chars = (ctx_tokens / 2) * 3;
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let q: serde_json::Value = serde_json::from_str(&line)?;
        let question = q["question"].as_str().unwrap_or_default();
        let route = rag::route(&lib, question);
        let p = rag::build_prompt(&lib, question, 5, max_chars);
        let cards: Vec<serde_json::Value> = match &route {
            rag::Route::Cards { card_ids, .. } => card_ids
                .iter()
                .filter_map(|id| lib.card(id))
                .map(|c| serde_json::json!({"id": c.id, "title": c.title, "text": c.full_text()}))
                .collect(),
            rag::Route::Generate => Vec::new(),
        };
        let out = serde_json::json!({
            "id": q["id"],
            "route": route,
            "cards": cards,
            "system": p.system,
            "user": p.user,
            "sources": p.passages.iter().map(|x| serde_json::json!({
                "key": x.key, "title": x.title, "source": x.source, "reviewed": x.reviewed
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string(&out)?);
    }
    Ok(())
}
