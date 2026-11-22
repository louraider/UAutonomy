use std::sync::{Arc, Mutex, OnceLock, RwLock};

use uautonomy_core::share::{ShareServer, ShareState};
use uautonomy_core::{content, Catalog, PackStore};

pub(crate) struct App {
    pub shared: Arc<RwLock<ShareState>>,
    pub server: Mutex<Option<ShareServer>>,
    pub catalog: Mutex<Option<Catalog>>,
    pub data_dir: std::path::PathBuf,
}

static APP: OnceLock<App> = OnceLock::new();

pub(crate) fn init(data_dir: &str) -> anyhow::Result<&'static App> {
    if let Some(a) = APP.get() {
        return Ok(a);
    }
    let store = PackStore::open(data_dir)?;
    let lib = content::builtin_base();
    let app = App {
        shared: Arc::new(RwLock::new(ShareState::new(store, lib))),
        server: Mutex::new(None),
        catalog: Mutex::new(None),
        data_dir: data_dir.into(),
    };
    let _ = APP.set(app);
    let a = app_ref()?;
    reload_library(a);
    Ok(a)
}

pub(crate) fn app_ref() -> anyhow::Result<&'static App> {
    APP.get()
        .ok_or_else(|| anyhow::anyhow!("бібліотеку ще не відкрито"))
}

/// Перечитує бібліотеку після встановлення чи видалення пакетів.
pub(crate) fn reload_library(a: &App) -> Vec<String> {
    let mut lib = content::builtin_base();
    let store = a.shared.read().unwrap().store.clone();
    let warnings = content::load_installed(&mut lib, &store);
    a.shared.write().unwrap().library = lib;
    warnings
}
