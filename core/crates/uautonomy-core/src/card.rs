use serde::{Deserialize, Serialize};

/// Тематичний розділ бібліотеки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

/// Коротка картка «що робити зараз». Картки входять у базовий пакет і
/// доступні одразу після встановлення, без завантажень.
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
}
