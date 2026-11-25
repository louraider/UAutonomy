//! Завантаження вмісту пакетів у [`Library`].

use std::fs;
use std::path::Path;
use std::sync::Arc;

use crate::card::{Article, Card};
use crate::pack::{PackKind, PackManifest};
use crate::search::Library;
use crate::store::PackStore;
use crate::zim::Zim;

/// Базовий пакет, вшитий у застосунок: картки (JSON-масив) і статті
/// (пари «ідентифікатор, Markdown»).
pub fn base_library(
    cards_json: &str,
    articles: &[(String, String)],
) -> Result<Library, crate::Error> {
    let cards: Vec<Card> = serde_json::from_str(cards_json)?;
    let mut lib = Library::new(cards);
    for (id, md) in articles {
        let a = Article::parse_markdown(id, md);
        let src = article_source(&a, "Базовий пакет");
        lib.add_article("base", a, &src);
    }
    Ok(lib)
}

/// Короткі назви джерел за id (`content/sources.json` → `short`).
pub fn source_names() -> &'static std::collections::HashMap<String, String> {
    static NAMES: std::sync::OnceLock<std::collections::HashMap<String, String>> =
        std::sync::OnceLock::new();
    NAMES.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/base/sources.json")).unwrap_or_default()
    })
}

/// Джерела статті для читача: назви замість внутрішніх id (`moz-water`).
/// Невідомі id не показуємо; якщо не лишилось нічого — `fallback`.
fn article_source(a: &Article, fallback: &str) -> String {
    let names: Vec<&str> = a
        .source_ids
        .iter()
        .filter_map(|id| source_names().get(id).map(String::as_str))
        .collect();
    if names.is_empty() {
        fallback.to_string()
    } else {
        names.join("; ")
    }
}

/// Додає вміст усіх встановлених пакетів. Повертає попередження про
/// пакети, які не вдалося прочитати (бібліотека при цьому працює далі).
///
/// Читаються лише пакети з дійсним підписом ([`PackStore::installed`]), від
/// старших підписів до новіших: картка чи стаття з тим самим id у новішому
/// пакеті замінює вбудовану або старішу.
pub fn load_installed(lib: &mut Library, store: &PackStore) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut packs = store.installed();
    packs.sort_by(|a, b| a.issued().cmp(&b.issued()).then(a.id.cmp(&b.id)));
    for m in packs {
        if let Err(e) = load_pack(lib, store, &m) {
            warnings.push(format!("{}: {e}", m.title));
        }
    }
    warnings
}

pub fn load_pack(
    lib: &mut Library,
    store: &PackStore,
    m: &PackManifest,
) -> Result<(), crate::Error> {
    if m.kind != PackKind::Content {
        return Ok(());
    }
    let dir = store.pack_dir(&m.id);
    for f in &m.files {
        let p = dir.join(&f.path);
        let lower = f.path.to_lowercase();
        if lower.ends_with(".json") && (lower == "cards.json" || lower.starts_with("cards/")) {
            let cards: Vec<Card> = serde_json::from_str(&fs::read_to_string(&p)?)?;
            lib.add_cards(&m.id, cards);
        } else if lower.ends_with(".md") && lower.starts_with("articles/") {
            let id = Path::new(&f.path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let a = Article::parse_markdown(&id, &fs::read_to_string(&p)?);
            let src = article_source(&a, &m.title);
            lib.add_article(&m.id, a, &src);
        } else if lower.ends_with(".zim") {
            let z = Zim::open(&p)?;
            lib.add_zim(&m.id, Arc::new(z));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_base_and_packs() {
        let mut lib = base_library(
            r#"[{"id":"b1","category":"first_aid","title":"Опік","steps":["Охолодіть"],"source":"тест"}]"#,
            &[("w".into(), "---\nid: water\ntitle: Вода\nsource_ids: [moz-water, nonexistent]\n---\nКип'ятіть.".into())],
        )
        .unwrap();
        assert_eq!(lib.search("вода", 3)[0].source, "МОЗ: знезараження води");
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("p");
        crate::store::tests::sample_pack(&src, "pocket");
        let store = crate::store::tests::test_store(&t.path().join("s"));
        store.install_from_dir(&src, &mut |_, _| {}).unwrap();
        assert!(load_installed(&mut lib, &store).is_empty());
        assert_eq!(lib.search("свічка", 3)[0].pack_id, "pocket");
        assert_eq!(lib.search("генератор", 3)[0].key, "article:gen");
    }

    /// Підписаний пакет виправляє вбудовану картку: у пошуку, у картці й
    /// у RAG лишається лише нова редакція.
    #[test]
    fn signed_pack_overrides_builtin_card() {
        let base = r#"[{"id":"fa-burn","category":"first_aid","title":"Опік","steps":["Стара порада: олія"],"source":"база","tags":["опік"]},
                       {"id":"fa-cut","category":"first_aid","title":"Поріз","steps":["Притисніть"],"source":"база"}]"#;
        let mut lib =
            base_library(base, &[("gen".into(), "# Генератор\nСтара стаття.".into())]).unwrap();
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("fix");
        fs::create_dir_all(src.join("articles")).unwrap();
        fs::write(
            src.join("cards.json"),
            r#"[{"id":"fa-burn","category":"first_aid","title":"Опік","steps":["Охолоджуйте водою 20 хвилин"],"source":"МОЗ 441","reviewed":true}]"#,
        )
        .unwrap();
        fs::write(
            src.join("articles/gen.md"),
            "# Генератор\nНова стаття: лише надворі.",
        )
        .unwrap();
        let m = crate::store::manifest_for_dir(
            &src,
            PackManifest {
                id: "fix-2026-11".into(),
                tier: crate::PackTier::Pocket,
                kind: PackKind::Content,
                title: "Виправлення".into(),
                description: String::new(),
                version: "1".into(),
                files: vec![],
                content_license: "CC-BY-4.0".into(),
                region: None,
                model_id: None,
                signature: None,
            },
        )
        .unwrap();
        fs::write(src.join("manifest.json"), m.to_json()).unwrap();
        crate::store::tests::sign_dir(&src, 5000);
        let store = crate::store::tests::test_store(&t.path().join("s"));
        store.install_from_dir(&src, &mut |_, _| {}).unwrap();
        assert!(load_installed(&mut lib, &store).is_empty());

        let c = lib.card("fa-burn").unwrap();
        assert_eq!(c.steps, vec!["Охолоджуйте водою 20 хвилин"]);
        assert!(c.reviewed);
        assert_eq!(lib.cards().iter().filter(|c| c.id == "fa-burn").count(), 1);
        assert!(lib.card("fa-cut").is_some(), "інші картки лишаються");
        let hits = lib.search("опік", 5);
        assert_eq!(hits.iter().filter(|h| h.key == "card:fa-burn").count(), 1);
        assert_eq!(hits[0].pack_id, "fix-2026-11");
        assert!(lib
            .retrieve("опік олія", 5)
            .iter()
            .all(|p| !p.text.contains("олія")));
        let a = lib.doc("article:gen").unwrap();
        assert!(a.text.contains("Нова стаття"), "{}", a.text);
        assert!(lib
            .search("поріз", 3)
            .iter()
            .any(|h| h.key == "card:fa-cut"));
    }
}

/// Вшитий у застосунок базовий пакет (збирається `tools/build_base_pack.py`
/// з теки `content/`).
pub fn builtin_base() -> Library {
    let articles: Vec<(String, String)> =
        serde_json::from_str(include_str!("../assets/base/articles.json"))
            .expect("вбудовані статті коректні");
    base_library(include_str!("../assets/base/cards.json"), &articles)
        .expect("вбудовані картки коректні")
}

#[cfg(test)]
mod builtin_tests {
    #[test]
    fn builtin_base_loads() {
        assert!(!super::builtin_base().cards().is_empty());
    }
}
