use uautonomy_core::search::DocKind;
use uautonomy_core::{text, Card, Category};

use crate::state;

/// Картка для показу в інтерфейсі.
pub struct CardView {
    pub id: String,
    pub category: String,
    pub title: String,
    pub summary: Option<String>,
    pub steps: Vec<String>,
    pub warnings: Vec<String>,
    pub source: String,
    pub reviewed: bool,
    pub review_required: Option<String>,
    /// Дослівний текст офіційного джерела (пункти наказу МОЗ № 441).
    pub source_quote: Option<String>,
    /// Адреса офіційного тексту.
    pub source_url: Option<String>,
}

impl From<&Card> for CardView {
    fn from(c: &Card) -> Self {
        CardView {
            id: c.id.clone(),
            category: c.category.slug().into(),
            title: c.title.clone(),
            summary: c.summary.clone(),
            steps: c.steps.clone(),
            warnings: c.warnings.clone(),
            source: match &c.source_ref {
                Some(r) => format!("{}, {}", c.source, r),
                None => c.source.clone(),
            },
            reviewed: c.reviewed,
            review_required: c.review_required.clone(),
            source_quote: c.source_quote.clone(),
            source_url: c.source_url.clone(),
        }
    }
}

pub struct CategoryView {
    pub slug: String,
    pub title: String,
    pub cards: Vec<CardView>,
}

pub struct HitView {
    /// `card:<id>`, `article:<id>` або `zim:<pack>/<path>`.
    pub key: String,
    pub kind: String,
    pub title: String,
    pub snippet: String,
    pub source: String,
    pub reviewed: bool,
}

pub struct DocView {
    pub key: String,
    pub title: String,
    /// Markdown: заголовки, списки, таблиці, **жирний**, посилання
    /// `[текст](card:<id>)` і `[текст](zim:<пакет>/<шлях>)`.
    pub body: String,
    pub source: String,
    pub reviewed: bool,
    /// Хто має перевірити: `medic` (медичні статті й енциклопедії), `deminer`.
    pub review_required: Option<String>,
}

/// Медична енциклопедія (WikiMed, Вікіпедія: медицина, MDWiki): її статті
/// позначаємо «Не перевірено лікарем», як і медичні картки.
fn medical_zim(pack: &str, z: &uautonomy_core::zim::Zim) -> bool {
    let name = format!(
        "{pack} {} {}",
        z.metadata("Name").unwrap_or_default(),
        z.metadata("Title").unwrap_or_default()
    )
    .to_lowercase();
    ["medic", "mdwiki", "wikimed", "медицин"]
        .iter()
        .any(|w| name.contains(w))
}

/// Прибирає перший заголовок, якщо він повторює назву (її вже видно вгорі).
fn drop_title_heading(md: &str, title: &str) -> String {
    let t = md.trim_start();
    if let Some(first) = t.lines().next() {
        let h = first.trim_start_matches('#').trim();
        if first.starts_with('#') && h.eq_ignore_ascii_case(title.trim()) {
            return t[first.len()..].trim_start().to_string();
        }
    }
    md.to_string()
}

pub struct LibraryInfo {
    pub core_version: String,
    pub cards: u32,
    pub documents: u32,
    pub zims: u32,
    pub warnings: Vec<String>,
}

#[flutter_rust_bridge::frb(sync)]
pub fn core_version() -> String {
    uautonomy_core::version().to_string()
}

/// Відкриває бібліотеку в теці даних застосунку. Викликати один раз на старті.
pub fn open_library(data_dir: String) -> anyhow::Result<LibraryInfo> {
    let a = state::init(&data_dir)?;
    let warnings = state::reload_library(a);
    Ok(library_info_inner(warnings))
}

#[flutter_rust_bridge::frb(sync)]
pub fn library_info() -> LibraryInfo {
    library_info_inner(Vec::new())
}

fn library_info_inner(warnings: Vec<String>) -> LibraryInfo {
    let Ok(a) = state::app_ref() else {
        return LibraryInfo {
            core_version: uautonomy_core::version().into(),
            cards: 0,
            documents: 0,
            zims: 0,
            warnings,
        };
    };
    let s = a.shared.read().unwrap();
    LibraryInfo {
        core_version: uautonomy_core::version().into(),
        cards: s.library.cards().len() as u32,
        documents: s.library.len() as u32,
        zims: s.library.zims().count() as u32,
        warnings,
    }
}

#[flutter_rust_bridge::frb(sync)]
pub fn search(query: String) -> Vec<HitView> {
    let Ok(a) = state::app_ref() else {
        return Vec::new();
    };
    a.shared
        .read()
        .unwrap()
        .library
        .search(&query, 40)
        .into_iter()
        .map(|h| HitView {
            key: h.key,
            kind: match h.kind {
                DocKind::Card => "card",
                DocKind::Article => "article",
                DocKind::Zim => "zim",
            }
            .into(),
            title: h.title,
            snippet: h.snippet,
            source: h.source,
            reviewed: h.reviewed,
        })
        .collect()
}

/// Термінові картки за розділами.
#[flutter_rust_bridge::frb(sync)]
pub fn categories() -> Vec<CategoryView> {
    let Ok(a) = state::app_ref() else {
        return Vec::new();
    };
    let s = a.shared.read().unwrap();
    Category::ALL
        .into_iter()
        .map(|c| CategoryView {
            slug: c.slug().into(),
            title: c.title_uk().into(),
            cards: s
                .library
                .cards()
                .iter()
                .filter(|x| x.category == c)
                .map(CardView::from)
                .collect(),
        })
        .filter(|c| !c.cards.is_empty())
        .collect()
}

#[flutter_rust_bridge::frb(sync)]
pub fn card(id: String) -> Option<CardView> {
    let a = state::app_ref().ok()?;
    let s = a.shared.read().unwrap();
    s.library.card(&id).map(CardView::from)
}

/// Стаття або сторінка ZIM за ключем з результатів пошуку.
pub fn document(key: String) -> Option<DocView> {
    let a = state::app_ref().ok()?;
    let s = a.shared.read().unwrap();
    if let Some(rest) = key.strip_prefix("zim:") {
        let (pack, path) = rest.split_once('/')?;
        let z = s.library.zim(pack)?;
        // Посилання часто ведуть на перенаправлення («Інфаркт» → «Інфаркт
        // міокарда»): назва й база відносних посилань — від цільової статті.
        let e = z.resolve(&z.get(path)?).ok()?;
        let (mime, data) = z.content(&e).ok()?;
        let raw = String::from_utf8_lossy(&data);
        let body = if mime.starts_with("text/html") {
            let base = e.url.clone();
            text::html_to_markdown(&raw, &|href| {
                text::resolve_zim_link(&base, href).map(|p| format!("zim:{pack}/{p}"))
            })
        } else {
            raw.into_owned()
        };
        return Some(DocView {
            key: key.clone(),
            body: drop_title_heading(&body, &e.title),
            title: e.title.clone(),
            source: z.metadata("Title").unwrap_or_else(|| pack.to_string()),
            reviewed: false,
            review_required: medical_zim(pack, z).then(|| "medic".to_string()),
        });
    }
    let d = s.library.doc(&key)?;
    let body = d.markdown.clone().unwrap_or_else(|| d.text.clone());
    Some(DocView {
        key: d.key.clone(),
        body: drop_title_heading(&body, &d.title),
        title: d.title.clone(),
        source: d.source.clone(),
        reviewed: d.reviewed,
        review_required: d.review_required.clone(),
    })
}

#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_default_user_utils();
}
