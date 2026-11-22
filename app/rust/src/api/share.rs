use uautonomy_core::share::{ShareServer, DEFAULT_PORT};

use crate::state;

pub struct ShareInfo {
    pub running: bool,
    pub port: u16,
    /// Адреси для QR-коду (перша — найімовірніша).
    pub urls: Vec<String>,
}

/// Вмикає роздачу бібліотеки в локальній мережі. `app_files` — шляхи до
/// інсталяторів застосунку (APK), які інші зможуть завантажити.
pub fn start_sharing(device_name: String, app_files: Vec<String>) -> anyhow::Result<ShareInfo> {
    let a = state::app_ref()?;
    let mut srv = a.server.lock().unwrap();
    {
        let mut s = a.shared.write().unwrap();
        s.device_name = device_name;
        s.app_files = app_files.into_iter().map(Into::into).collect();
    }
    if srv.is_none() {
        let server = ShareServer::start(a.shared.clone(), DEFAULT_PORT)
            .or_else(|_| ShareServer::start(a.shared.clone(), 0))?;
        *srv = Some(server);
    }
    let s = srv.as_ref().unwrap();
    Ok(ShareInfo {
        running: true,
        port: s.port(),
        urls: s.urls(),
    })
}

pub fn stop_sharing() {
    if let Ok(a) = state::app_ref() {
        if let Some(s) = a.server.lock().unwrap().take() {
            s.stop();
        }
    }
}

#[flutter_rust_bridge::frb(sync)]
pub fn sharing_status() -> ShareInfo {
    let Ok(a) = state::app_ref() else {
        return ShareInfo {
            running: false,
            port: 0,
            urls: Vec::new(),
        };
    };
    match a.server.lock().unwrap().as_ref() {
        Some(s) => ShareInfo {
            running: true,
            port: s.port(),
            urls: s.urls(),
        },
        None => ShareInfo {
            running: false,
            port: 0,
            urls: Vec::new(),
        },
    }
}
