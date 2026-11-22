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

fn article_source(a: &Article, fallback: &str) -> String {
    if a.source_ids.is_empty() {
        fallback.to_string()
    } else {
        a.source_ids.join(", ")
    }
}

/// Додає вміст усіх встановлених пакетів. Повертає попередження про
/// пакети, які не вдалося прочитати (бібліотека при цьому працює далі).
pub fn load_installed(lib: &mut Library, store: &PackStore) -> Vec<String> {
    let mut warnings = Vec::new();
    for m in store.installed() {
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
            &[("w".into(), "---\nid: water\ntitle: Вода\nsource_ids: [dsns]\n---\nКип'ятіть.".into())],
        )
        .unwrap();
        assert_eq!(lib.search("вода", 3)[0].source, "dsns");
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("p");
        crate::store::tests::sample_pack(&src, "pocket");
        let store = PackStore::open(t.path().join("s")).unwrap();
        store.install_from_dir(&src, &mut |_, _| {}).unwrap();
        assert!(load_installed(&mut lib, &store).is_empty());
        assert_eq!(lib.search("свічка", 3)[0].pack_id, "pocket");
        assert_eq!(lib.search("генератор", 3)[0].key, "article:gen");
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
