//! Тонкий міст між Flutter і `uautonomy-core`. Уся логіка живе в ядрі,
//! тут лише перетворення типів для Dart.

use uautonomy_core::{Card, Category, Library};

/// Картка для показу в інтерфейсі.
pub struct CardView {
    pub title: String,
    pub steps: Vec<String>,
    pub source: String,
    pub reviewed: bool,
}

#[flutter_rust_bridge::frb(sync)]
pub fn core_version() -> String {
    uautonomy_core::version().to_string()
}

/// Пошук по вбудованих демонстраційних картках.
#[flutter_rust_bridge::frb(sync)]
pub fn search_cards(query: String) -> Vec<CardView> {
    demo_library()
        .search(&query)
        .into_iter()
        .map(|c| CardView {
            title: c.title.clone(),
            steps: c.steps.clone(),
            source: c.source.clone(),
            reviewed: c.reviewed,
        })
        .collect()
}

/// Тимчасові картки-заглушки, щоб перевірити ланцюжок Flutter → Rust.
/// Це НЕ медичні рекомендації: справжній вміст надходить лише з перевірених
/// пакетів після рецензії фахівців.
fn demo_library() -> Library {
    Library::new(vec![
        Card {
            id: "demo-112".into(),
            category: Category::CivilProtection,
            title: "Виклик допомоги".into(),
            steps: vec!["Телефонуйте 112, 101 або 103.".into()],
            source: "Демонстраційна картка".into(),
            reviewed: false,
        },
        Card {
            id: "demo-pack".into(),
            category: Category::Household,
            title: "Тривожна валіза".into(),
            steps: vec!["Документи, вода, ліки, ліхтарик, павербанк.".into()],
            source: "Демонстраційна картка".into(),
            reviewed: false,
        },
    ])
}

#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_default_user_utils();
}
