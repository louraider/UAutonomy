use serde::{Deserialize, Serialize};

/// Тематичний розділ бібліотеки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Домедична допомога (Наказ МОЗ №441, IFRC First Aid Guidelines 2025).
    FirstAid,
    /// Цивільний захист (матеріали ДСНС).
    CivilProtection,
    /// Мінна безпека. Лише перевірені фахівцями матеріали.
    MineSafety,
    /// Вода, їжа, тепло, енергія під час блекауту.
    Household,
    /// Зв'язок і орієнтування.
    Communication,
}

impl Category {
    pub const ALL: [Category; 5] = [
        Category::FirstAid,
        Category::CivilProtection,
        Category::MineSafety,
        Category::Household,
        Category::Communication,
    ];

    /// Назва розділу для інтерфейсу.
    pub fn title_uk(self) -> &'static str {
        match self {
            Category::FirstAid => "Домедична допомога",
            Category::CivilProtection => "Цивільний захист",
            Category::MineSafety => "Мінна безпека",
            Category::Household => "Побут і блекаут",
            Category::Communication => "Зв'язок і орієнтування",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Category::FirstAid => "first_aid",
            Category::CivilProtection => "civil_protection",
            Category::MineSafety => "mine_safety",
            Category::Household => "household",
            Category::Communication => "communication",
        }
    }

    pub fn from_slug(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.slug() == s)
    }
}

/// Коротка картка «що робити зараз». Картки входять у базовий пакет і
/// доступні одразу після встановлення, без завантажень.
///
/// Формат збігається з `content/cards/<category>.json`: зайві поля serde
/// пропускає, тож контент може випереджати ядро.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub id: String,
    pub category: Category,
    pub title: String,
    /// Кроки дій, по одному на рядок.
    pub steps: Vec<String>,
    /// Звідки взято текст (назва документа, редакція).
    pub source: String,
    /// Чи пройшла картка перевірку фахівцем (медик, сапер). Неперевірені
    /// картки застосунок позначає попередженням.
    #[serde(default)]
    pub reviewed: bool,
    /// Одне речення: коли ця картка потрібна.
    #[serde(default)]
    pub summary: Option<String>,
    /// Чого НЕ робити.
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Посилання на `content/sources.json`.
    #[serde(default)]
    pub source_ids: Vec<String>,
    /// Точне місце в джерелі, напр. «Порядок 4, п. 4».
    #[serde(default)]
    pub source_ref: Option<String>,
    /// Хто має перевірити: "medic", "deminer" або нічого.
    #[serde(default)]
    pub review_required: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    /// Дослівний текст офіційного джерела, напр. пункти наказу МОЗ № 441.
    /// Застосунок показує його під кроками, щоб людина бачила першоджерело.
    #[serde(default)]
    pub source_quote: Option<String>,
    /// Адреса офіційного тексту, з якого взято `source_quote`.
    #[serde(default)]
    pub source_url: Option<String>,
}

impl Card {
    /// Мінімальний конструктор для тестів і демо.
    pub fn simple(id: &str, category: Category, title: &str, steps: &[&str], source: &str) -> Self {
        Card {
            id: id.into(),
            category,
            title: title.into(),
            steps: steps.iter().map(|s| s.to_string()).collect(),
            source: source.into(),
            reviewed: false,
            summary: None,
            warnings: Vec::new(),
            tags: Vec::new(),
            source_ids: Vec::new(),
            source_ref: None,
            review_required: None,
            updated: None,
            source_quote: None,
            source_url: None,
        }
    }

    /// Медична картка: домедична допомога або будь-яка, яку має перевірити медик.
    pub fn is_medical(&self) -> bool {
        self.category == Category::FirstAid || self.review_required.as_deref() == Some("medic")
    }

    /// Картка мінної безпеки (перевіряє сапер).
    pub fn is_mine_safety(&self) -> bool {
        self.category == Category::MineSafety || self.review_required.as_deref() == Some("deminer")
    }

    /// Увесь текст картки одним рядком (для пошуку й ШІ).
    pub fn full_text(&self) -> String {
        let mut s = String::new();
        if let Some(sum) = &self.summary {
            s.push_str(sum);
            s.push('\n');
        }
        for (i, step) in self.steps.iter().enumerate() {
            s.push_str(&format!("{}. {}\n", i + 1, step));
        }
        if !self.warnings.is_empty() {
            s.push_str("Не робіть: ");
            s.push_str(&self.warnings.join("; "));
            s.push('\n');
        }
        s
    }
}

/// Довідкова стаття з `content/articles/<category>/<id>.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Article {
    pub id: String,
    pub title: String,
    pub category: Option<Category>,
    pub source_ids: Vec<String>,
    pub review_required: Option<String>,
    pub reviewed: bool,
    /// Текст у Markdown без фронтматера.
    pub body: String,
}

impl Article {
    /// Розбирає Markdown з простим YAML-фронтматером (`key: value`,
    /// списки як `[a, b]` або рядками `- a`).
    pub fn parse_markdown(fallback_id: &str, text: &str) -> Article {
        let mut a = Article {
            id: fallback_id.to_string(),
            title: String::new(),
            category: None,
            source_ids: Vec::new(),
            review_required: None,
            reviewed: false,
            body: text.to_string(),
        };
        let t = text.trim_start_matches('\u{feff}');
        let Some(rest) = t.strip_prefix("---") else {
            a.title = first_heading(t).unwrap_or_else(|| fallback_id.to_string());
            return a;
        };
        let Some(end) = rest.find("\n---") else {
            return a;
        };
        let front = &rest[..end];
        let body = rest[end + 4..].trim_start_matches(['\r', '\n']);
        let mut current_list: Option<String> = None;
        for line in front.lines() {
            let line = line.trim_end();
            if let Some(item) = line.trim_start().strip_prefix("- ") {
                if current_list.as_deref() == Some("source_ids") {
                    a.source_ids.push(unquote(item));
                }
                continue;
            }
            let Some((k, v)) = line.split_once(':') else {
                continue;
            };
            let k = k.trim();
            let v = v.trim();
            current_list = if v.is_empty() {
                Some(k.to_string())
            } else {
                None
            };
            match k {
                "id" => a.id = unquote(v),
                "title" => a.title = unquote(v),
                "category" => a.category = Category::from_slug(&unquote(v)),
                "reviewed" => a.reviewed = v == "true",
                "review_required" => {
                    let v = unquote(v);
                    a.review_required = if v.is_empty() || v == "null" || v == "~" {
                        None
                    } else {
                        Some(v)
                    }
                }
                "source_ids" if v.starts_with('[') => {
                    a.source_ids = v
                        .trim_matches(['[', ']'])
                        .split(',')
                        .map(unquote)
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                _ => {}
            }
        }
        a.body = body.to_string();
        if a.title.is_empty() {
            a.title = first_heading(body).unwrap_or_else(|| a.id.clone());
        }
        a
    }
}

fn unquote(s: &str) -> String {
    s.trim().trim_matches(['"', '\'']).to_string()
}

fn first_heading(md: &str) -> Option<String> {
    md.lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(|s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_accepts_content_fields() {
        let json = r#"{"id":"fa-bleeding","category":"first_aid","title":"Кровотеча",
            "steps":["Тисніть"],"source":"Наказ 441","reviewed":false,
            "summary":"Коли","warnings":["Не знімайте турнікет"],"tags":["кров"],
            "source_ids":["moz-441"],"source_ref":"Порядок 4","review_required":"medic",
            "updated":"2026-11-22","unknown_field":1}"#;
        let c: Card = serde_json::from_str(json).unwrap();
        assert_eq!(c.review_required.as_deref(), Some("medic"));
        assert!(c.full_text().contains("Не робіть"));
    }

    #[test]
    fn parses_frontmatter() {
        let md = "---\nid: water-boil\ntitle: \"Кип'ятіння води\"\ncategory: household\nsource_ids:\n  - dsns-water\nreview_required: null\nreviewed: false\n---\n\n# Кип'ятіння\nТекст.";
        let a = Article::parse_markdown("x", md);
        assert_eq!(a.id, "water-boil");
        assert_eq!(a.title, "Кип'ятіння води");
        assert_eq!(a.category, Some(Category::Household));
        assert_eq!(a.source_ids, vec!["dsns-water"]);
        assert_eq!(a.review_required, None);
        assert!(a.body.starts_with("# Кип"));
    }
}
