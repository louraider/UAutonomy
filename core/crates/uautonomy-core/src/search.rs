//! Офлайн-пошук по бібліотеці: BM25 по картках і статтях базового пакета
//! та завантажених пакетів, плюс пошук за заголовками у файлах ZIM.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::card::{Article, Card, Category};
use crate::text;
use crate::zim::Zim;

/// Що саме знайдено.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocKind {
    Card,
    Article,
    Zim,
}

/// Документ бібліотеки (картка або стаття).
#[derive(Debug, Clone)]
pub struct Doc {
    /// Унікальний ідентифікатор: `card:<id>`, `article:<id>`, `zim:<pack>/<path>`.
    pub key: String,
    pub kind: DocKind,
    pub title: String,
    /// Простий текст для пошуку, уривків і ШІ.
    pub text: String,
    /// Markdown для показу (статті); `None` — показувати `text`.
    pub markdown: Option<String>,
    pub source: String,
    pub reviewed: bool,
    pub category: Option<Category>,
    /// Хто має перевірити: `medic`, `deminer` або `None`.
    pub review_required: Option<String>,
    pub pack_id: String,
}

#[derive(Debug, Clone)]
struct Chunk {
    doc: usize,
    text: String,
    tf: HashMap<String, u32>,
    len: u32,
}

/// Результат пошуку.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchHit {
    pub key: String,
    pub kind: DocKind,
    pub title: String,
    pub snippet: String,
    pub source: String,
    pub reviewed: bool,
    pub pack_id: String,
    pub score: f32,
}

/// Фрагмент тексту для ШІ (RAG).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Passage {
    pub key: String,
    pub title: String,
    pub text: String,
    pub source: String,
    pub reviewed: bool,
    pub score: f32,
}

const CHUNK_CHARS: usize = 900;
const K1: f32 = 1.2;
const B: f32 = 0.75;
const TITLE_WEIGHT: u32 = 3;
const TITLE_BONUS: f32 = 4.0;

#[derive(Default)]
pub struct Library {
    cards: Vec<Card>,
    docs: Vec<Doc>,
    chunks: Vec<Chunk>,
    df: HashMap<String, u32>,
    total_len: u64,
    zims: Vec<(String, Arc<Zim>)>,
}

impl std::fmt::Debug for Library {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Library")
            .field("docs", &self.docs.len())
            .field("chunks", &self.chunks.len())
            .field("zims", &self.zims.len())
            .finish()
    }
}

impl Library {
    pub fn new(cards: Vec<Card>) -> Self {
        let mut lib = Library::default();
        lib.add_cards("base", cards);
        lib
    }

    pub fn len(&self) -> usize {
        self.docs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    pub fn card(&self, id: &str) -> Option<&Card> {
        self.cards.iter().find(|c| c.id == id)
    }

    pub fn doc(&self, key: &str) -> Option<&Doc> {
        self.docs.iter().find(|d| d.key == key)
    }

    pub fn zims(&self) -> impl Iterator<Item = (&str, &Arc<Zim>)> {
        self.zims.iter().map(|(p, z)| (p.as_str(), z))
    }

    pub fn zim(&self, pack_id: &str) -> Option<&Arc<Zim>> {
        self.zims.iter().find(|(p, _)| p == pack_id).map(|(_, z)| z)
    }

    /// Додає картки. Картка з уже наявним `id` замінює попередню: так
    /// підписаний пакет виправляє вбудовану картку без нового APK. Пакети
    /// завантажуються від старших підписів до новіших, тож перемагає
    /// найновіша редакція.
    pub fn add_cards(&mut self, pack_id: &str, cards: Vec<Card>) {
        let replaced: std::collections::HashSet<String> = cards
            .iter()
            .filter(|c| self.card(&c.id).is_some())
            .map(|c| format!("card:{}", c.id))
            .collect();
        if !replaced.is_empty() {
            self.rebuild_without(&|d| replaced.contains(&d.key));
        }
        for c in cards {
            if self.cards.iter().any(|x| x.id == c.id) {
                continue; // дубль у самому пакеті: лишається перша
            }
            let doc = Doc {
                key: format!("card:{}", c.id),
                kind: DocKind::Card,
                title: c.title.clone(),
                text: c.full_text(),
                markdown: None,
                source: card_source(&c),
                reviewed: c.reviewed,
                category: Some(c.category),
                review_required: c.review_required.clone(),
                pack_id: pack_id.to_string(),
            };
            self.add_doc(doc, &c.tags.join(" "));
            self.cards.push(c);
        }
    }

    pub fn add_article(&mut self, pack_id: &str, a: Article, source: &str) {
        let key = format!("article:{}", a.id);
        if self.doc(&key).is_some() {
            // Новіша редакція статті замінює попередню (як і для карток).
            self.rebuild_without(&|d| d.key == key);
        }
        // Редакторські коментарі прибираємо, ідентифікатори карток у
        // таблицях стають посиланнями з назвами карток.
        let markdown = text::link_card_ids(&text::strip_comments(&a.body), &|id| {
            self.card(id).map(|c| c.title.clone())
        });
        self.add_doc(
            Doc {
                key,
                kind: DocKind::Article,
                title: a.title,
                text: text::markdown_to_plain(&markdown),
                markdown: Some(markdown),
                source: source.to_string(),
                reviewed: a.reviewed,
                category: a.category,
                review_required: a.review_required,
                pack_id: pack_id.to_string(),
            },
            "",
        );
    }

    pub fn add_zim(&mut self, pack_id: &str, zim: Arc<Zim>) {
        self.zims.retain(|(p, _)| p != pack_id);
        self.zims.push((pack_id.to_string(), zim));
    }

    /// Прибирає все, що прийшло з пакета (після видалення пакета).
    pub fn remove_pack(&mut self, pack_id: &str) {
        self.rebuild_without(&|d| d.pack_id == pack_id);
        self.zims.retain(|(p, _)| p != pack_id);
    }

    /// Перебудовує індекс без документів, для яких `drop` повертає true
    /// (разом із їхніми картками). Порядок решти зберігається.
    fn rebuild_without(&mut self, drop: &dyn Fn(&Doc) -> bool) {
        let docs = std::mem::take(&mut self.docs);
        let mut cards: HashMap<String, Card> = std::mem::take(&mut self.cards)
            .into_iter()
            .map(|c| (c.id.clone(), c))
            .collect();
        let zims = std::mem::take(&mut self.zims);
        *self = Library::default();
        self.zims = zims;
        for d in docs {
            let card = match d.kind {
                DocKind::Card => cards.remove(d.key.trim_start_matches("card:")),
                _ => None,
            };
            if drop(&d) {
                continue;
            }
            let extra = card.as_ref().map(|c| c.tags.join(" ")).unwrap_or_default();
            if let Some(c) = card {
                self.cards.push(c);
            }
            self.add_doc(d, &extra);
        }
    }

    fn add_doc(&mut self, doc: Doc, extra: &str) {
        let idx = self.docs.len();
        let title_tokens = text::tokens(&format!("{} {}", doc.title, extra));
        for (i, part) in text::chunks(&doc.text, CHUNK_CHARS).into_iter().enumerate() {
            let mut tf: HashMap<String, u32> = HashMap::new();
            let toks = text::tokens(&part);
            let mut len = toks.len() as u32;
            for t in toks {
                *tf.entry(t).or_default() += 1;
            }
            // Заголовок важить більше; додаємо його до першого фрагмента.
            if i == 0 {
                for t in &title_tokens {
                    *tf.entry(t.clone()).or_default() += TITLE_WEIGHT;
                    len += TITLE_WEIGHT;
                }
            }
            for t in tf.keys() {
                *self.df.entry(t.clone()).or_default() += 1;
            }
            self.total_len += len as u64;
            self.chunks.push(Chunk {
                doc: idx,
                text: part,
                tf,
                len,
            });
        }
        if self.chunks.last().is_none_or(|c| c.doc != idx) {
            // Порожній текст: індексуємо хоча б заголовок.
            let mut tf: HashMap<String, u32> = HashMap::new();
            for t in &title_tokens {
                *tf.entry(t.clone()).or_default() += TITLE_WEIGHT;
            }
            for t in tf.keys() {
                *self.df.entry(t.clone()).or_default() += 1;
            }
            let len = tf.values().sum();
            self.total_len += len as u64;
            self.chunks.push(Chunk {
                doc: idx,
                text: String::new(),
                tf,
                len,
            });
        }
        self.docs.push(doc);
    }

    fn bm25(&self, q: &[String], tf: &HashMap<String, u32>, len: u32) -> f32 {
        let n = self.chunks.len().max(1) as f32;
        let avg = (self.total_len as f32 / n).max(1.0);
        q.iter()
            .map(|t| {
                let f = *tf.get(t).unwrap_or(&0) as f32;
                if f == 0.0 {
                    return 0.0;
                }
                let df = *self.df.get(t).unwrap_or(&1) as f32;
                let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
                idf * f * (K1 + 1.0) / (f + K1 * (1.0 - B + B * len as f32 / avg))
            })
            .sum()
    }

    /// Найкращі фрагменти за BM25: (номер фрагмента, бал).
    fn top_chunks(&self, q: &[String], limit: usize) -> Vec<(usize, f32)> {
        let mut scored: Vec<(usize, f32)> = self
            .chunks
            .iter()
            .enumerate()
            .map(|(i, c)| (i, self.bm25(q, &c.tf, c.len)))
            .filter(|(_, s)| *s > 0.0)
            .collect();
        // Збіг із заголовком — найсильніший сигнал: «кровотеча» має бути
        // вище за довгу картку, де кровотеча лише згадується.
        for (i, s) in scored.iter_mut() {
            let d = &self.docs[self.chunks[*i].doc];
            *s += TITLE_BONUS * title_overlap(&d.title, q);
        }
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        scored.truncate(limit);
        scored
    }

    /// Пошук для інтерфейсу: один результат на документ.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let q = text::tokens(query);
        if q.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<SearchHit> = Vec::new();
        for (ci, score) in self.top_chunks(&q, limit * 4) {
            let c = &self.chunks[ci];
            let d = &self.docs[c.doc];
            if hits.iter().any(|h| h.key == d.key) {
                continue;
            }
            hits.push(SearchHit {
                key: d.key.clone(),
                kind: d.kind,
                title: d.title.clone(),
                snippet: snippet(&c.text, &q),
                source: d.source.clone(),
                reviewed: d.reviewed,
                pack_id: d.pack_id.clone(),
                score,
            });
        }
        for (pack, zim, e) in self.zim_title_hits(query, &q, limit) {
            let key = format!("zim:{pack}/{}", e.path());
            if hits.iter().any(|h| h.key == key) {
                continue;
            }
            let overlap = title_overlap(&e.title, &q);
            hits.push(SearchHit {
                key,
                kind: DocKind::Zim,
                title: e.title.clone(),
                snippet: String::new(),
                source: zim.metadata("Title").unwrap_or_else(|| pack.clone()),
                reviewed: false,
                pack_id: pack,
                score: overlap,
            });
        }
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        hits.truncate(limit);
        hits
    }

    fn zim_title_hits(
        &self,
        raw: &str,
        q: &[String],
        limit: usize,
    ) -> Vec<(String, Arc<Zim>, crate::zim::Entry)> {
        let mut out: Vec<(String, Arc<Zim>, crate::zim::Entry)> = Vec::new();
        if self.zims.is_empty() {
            return out;
        }
        let mut variants: Vec<String> = vec![raw.trim().to_string(), capitalize(raw.trim())];
        for w in raw.split_whitespace().filter(|w| w.chars().count() >= 4) {
            let w = w.trim_matches(|c: char| !c.is_alphanumeric());
            variants.push(capitalize(w));
            // Основа слова ловить інші відмінки: «Кровотечі» → «Кровотеч».
            let st = text::stem(&text::normalize(w));
            if st.chars().count() >= 4 {
                variants.push(capitalize(&st));
            }
        }
        variants.retain(|v| !v.is_empty());
        variants.dedup();
        for (pack, zim) in &self.zims {
            let ns = zim.content_namespace();
            for v in &variants {
                for e in zim.find_by_title_prefix(ns, v, limit) {
                    if out.iter().any(|(p, _, x)| p == pack && x.index == e.index) {
                        continue;
                    }
                    if title_overlap(&e.title, q) > 0.0 {
                        out.push((pack.clone(), zim.clone(), e));
                    }
                }
            }
        }
        out
    }

    /// Фрагменти для відповіді ШІ: найкращі шматки карток і статей плюс
    /// найкращий шматок з кожної знайденої статті ZIM.
    pub fn retrieve(&self, query: &str, k: usize) -> Vec<Passage> {
        let q = text::tokens(query);
        if q.is_empty() {
            return Vec::new();
        }
        let mut out: Vec<Passage> = Vec::new();
        for (ci, score) in self.top_chunks(&q, k * 3) {
            let c = &self.chunks[ci];
            let d = &self.docs[c.doc];
            if out.iter().filter(|p| p.key == d.key).count() >= 2 {
                continue;
            }
            let text = if c.text.is_empty() {
                d.text.clone()
            } else {
                c.text.clone()
            };
            out.push(Passage {
                key: d.key.clone(),
                title: d.title.clone(),
                text,
                source: d.source.clone(),
                reviewed: d.reviewed,
                score,
            });
        }
        for (pack, zim, e) in self.zim_title_hits(query, &q, 3) {
            let Ok((mime, data)) = zim.content(&e) else {
                continue;
            };
            if !mime.starts_with("text/html") {
                continue;
            }
            let plain = text::html_to_text(&String::from_utf8_lossy(&data));
            let best = text::chunks(&plain, CHUNK_CHARS)
                .into_iter()
                .map(|ch| {
                    let toks = text::tokens(&ch);
                    let len = toks.len() as u32;
                    let mut tf: HashMap<String, u32> = HashMap::new();
                    for t in toks {
                        *tf.entry(t).or_default() += 1;
                    }
                    let s = self.bm25(&q, &tf, len) + title_overlap(&e.title, &q);
                    (ch, s)
                })
                .max_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((text, score)) = best {
                out.push(Passage {
                    key: format!("zim:{pack}/{}", e.path()),
                    title: e.title.clone(),
                    text,
                    source: zim.metadata("Title").unwrap_or(pack.clone()),
                    reviewed: false,
                    score,
                });
            }
        }
        out.sort_by(|a, b| b.score.total_cmp(&a.score));
        out.truncate(k);
        out
    }
}

fn card_source(c: &Card) -> String {
    match &c.source_ref {
        Some(r) if !r.is_empty() => format!("{}, {}", c.source, r),
        _ => c.source.clone(),
    }
}

fn capitalize(s: &str) -> String {
    let mut ch = s.chars();
    match ch.next() {
        Some(f) => f.to_uppercase().chain(ch).collect(),
        None => String::new(),
    }
}

fn title_overlap(title: &str, q: &[String]) -> f32 {
    let t = text::tokens(title);
    if t.is_empty() {
        return 0.0;
    }
    let common = q.iter().filter(|w| t.contains(w)).count() as f32;
    common / q.len() as f32
}

/// Рядок навколо першого збігу.
fn snippet(text: &str, q: &[String]) -> String {
    for line in text.lines() {
        let toks = crate::text::tokens(line);
        if q.iter().any(|w| toks.contains(w)) {
            return truncate(line, 200);
        }
    }
    truncate(text.lines().next().unwrap_or(""), 200)
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zim::writer::{write_zim, Item};

    fn lib() -> Library {
        Library::new(vec![
            Card::simple(
                "bleed",
                Category::FirstAid,
                "Сильна кровотеча",
                &["Накладіть турнікет вище рани"],
                "тест",
            ),
            Card::simple(
                "burn",
                Category::FirstAid,
                "Опік",
                &["Охолодіть водою 20 хвилин"],
                "тест",
            ),
            Card::simple(
                "water",
                Category::Household,
                "Питна вода",
                &["Кип'ятіть воду щонайменше хвилину"],
                "тест",
            ),
        ])
    }

    #[test]
    fn finds_inflected_words() {
        let l = lib();
        let hits = l.search("ТУРНІКЕТА", 5);
        assert_eq!(hits[0].key, "card:bleed");
        let hits = l.search("що робити при кровотечі", 5);
        assert_eq!(hits[0].key, "card:bleed");
        assert!(l.search("   ", 5).is_empty());
        let hits = l.search("вода", 5);
        assert_eq!(hits[0].key, "card:water", "заголовок важить більше");
    }

    #[test]
    fn retrieves_from_zim_and_removes_packs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("w.zim");
        let mut f = std::fs::File::create(&path).unwrap();
        write_zim(
            vec![Item {
                namespace: 'C',
                url: "Знезараження_води".into(),
                title: "Знезараження води".into(),
                mime: "text/html".into(),
                data: "<p>Хлорні таблетки: одна на літр.</p>".as_bytes().to_vec(),
            }],
            None,
            &mut f,
        )
        .unwrap();
        drop(f);
        let mut l = lib();
        l.add_zim("wiki", Arc::new(Zim::open(&path).unwrap()));
        l.add_article(
            "pocket",
            Article::parse_markdown("a1", "# Генератор\nНе запускайте генератор у приміщенні."),
            "ДСНС",
        );
        let p = l.retrieve("знезараження води", 5);
        assert!(p
            .iter()
            .any(|x| x.key == "zim:wiki/C/Знезараження_води" && x.text.contains("таблетки")));
        assert!(l.search("генератор", 3)[0].key == "article:a1");
        l.remove_pack("pocket");
        l.remove_pack("wiki");
        assert!(l.search("генератор", 3).is_empty());
        assert_eq!(l.cards().len(), 3);
        assert!(l
            .retrieve("знезараження", 5)
            .iter()
            .all(|x| !x.key.starts_with("zim:")));
    }
}
