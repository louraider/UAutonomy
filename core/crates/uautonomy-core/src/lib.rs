//! Ядро «Автономії».
//!
//! Цей crate не залежить від UI: його використовують і Flutter-застосунок
//! (через `flutter_rust_bridge`), і headless-хаб (`uautonomy-hub`) для Raspberry Pi
//! та домашніх серверів.
//!
//! Зараз тут лише каркас: опис пакетів контенту та простий пошук по картках.
//! Далі сюди додаються модулі `zim` (libzim), `maps` (PMTiles/OSM), `ai`
//! (llama.cpp / LiteRT-LM) і `share` (обмін телефон-телефон, локальний Wi-Fi-сервер).

pub mod card;
pub mod pack;
pub mod search;

pub use card::{Card, Category};
pub use pack::{PackManifest, PackTier};
pub use search::Library;

/// Версія ядра, яку показує застосунок.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("невірний маніфест пакета: {0}")]
    Manifest(#[from] serde_json::Error),
}
