//! Ядро «Автономії».
//!
//! Цей crate не залежить від UI: його використовують і Flutter-застосунок
//! (через `flutter_rust_bridge`), і headless-хаб (`uautonomy-hub`) для
//! Raspberry Pi та домашніх серверів.
//!
//! | Модуль | Що робить |
//! |---|---|
//! | [`card`], [`content`] | картки, статті, завантаження вмісту пакетів |
//! | [`zim`] | читач довідників ZIM (Kiwix) на чистому Rust |
//! | [`search`], [`text`] | офлайн-пошук BM25 з грубим українським стемінгом |
//! | [`pack`], [`store`], [`download`] | пакети, SHA-256, докачка, імпорт з USB |
//! | [`hardware`], [`models`] | можливості пристрою, вибір моделі ШІ |
//! | [`rag`] | запит до локальної моделі з джерелами |
//! | [`regions`] | області для офлайн-карт |
//! | [`share`] | Wi-Fi-сервер для інших телефонів і хаба |
//! | [`webmap`] | карти в браузері через Wi-Fi-обмін (iPhone, ноутбук) |

pub mod card;
pub mod content;
pub mod download;
pub mod hardware;
pub mod models;
pub mod pack;
pub mod pmtiles;
pub mod rag;
pub mod regions;
pub mod search;
pub mod share;
pub mod store;
pub mod text;
pub mod webmap;
pub mod zim;

pub use card::{Article, Card, Category};
pub use pack::{Catalog, PackFile, PackKind, PackManifest, PackTier};
pub use search::{Library, Passage, SearchHit};
pub use store::PackStore;

/// Версія ядра, яку показує застосунок.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("невірний JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("помилка файлу: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Invalid(String),
    #[error("файл {0} пошкоджено або підмінено (не збігається SHA-256)")]
    Checksum(String),
    #[error("помилка мережі: {0}")]
    Network(String),
    #[error("скасовано")]
    Cancelled,
    #[error(transparent)]
    Zim(#[from] zim::ZimError),
}
