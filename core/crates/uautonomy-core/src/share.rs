//! Локальний Wi-Fi-сервер: телефон, ПК чи хаб роздає бібліотеку іншим
//! пристроям у тій самій мережі (точка доступу телефона, домашній роутер).
//! Інші телефони відкривають адресу з QR-коду у браузері або в застосунку
//! «Автономія» й можуть завантажити пакети та сам застосунок (APK).
//!
//! Сервер лише читає: нічого не приймає й не змінює. Без TLS, бо працює
//! без інтернету в локальній мережі.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::thread::JoinHandle;

use serde::Serialize;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::pack::{is_safe_id, is_safe_rel_path, Catalog, PackKind};
use crate::pmtiles::PmTiles;
use crate::search::Library;
use crate::store::PackStore;
use crate::webmap::{self, MapInfo};

pub const DEFAULT_PORT: u16 = 8642;

/// Що роздає сервер.
pub struct ShareState {
    pub store: PackStore,
    pub library: Library,
    /// Теки з інсталяторами застосунку (APK, Windows-zip), що роздаються як `/app/<файл>`.
    pub app_files: Vec<PathBuf>,
    /// Назва пристрою для заголовка сторінки.
    pub device_name: String,
}

impl ShareState {
    pub fn new(store: PackStore, library: Library) -> Self {
        ShareState {
            store,
            library,
            app_files: Vec::new(),
            device_name: "Автономія".into(),
        }
    }
}

pub struct ShareServer {
    server: Arc<Server>,
    port: u16,
    workers: Vec<JoinHandle<()>>,
}

impl ShareServer {
    /// `port = 0` — будь-який вільний.
    pub fn start(state: Arc<RwLock<ShareState>>, port: u16) -> Result<ShareServer, crate::Error> {
        let server =
            Server::http(("0.0.0.0", port)).map_err(|e| crate::Error::Network(e.to_string()))?;
        let port = server
            .server_addr()
            .to_ip()
            .map(|a| a.port())
            .ok_or_else(|| crate::Error::Network("невідома адреса".into()))?;
        let server = Arc::new(server);
        let workers = (0..4)
            .map(|_| {
                let s = server.clone();
                let st = state.clone();
                std::thread::spawn(move || {
                    for req in s.incoming_requests() {
                        handle(&st, req);
                    }
                })
            })
            .collect();
        Ok(ShareServer {
            server,
            port,
            workers,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Адреси, які можна показати в QR-коді.
    pub fn urls(&self) -> Vec<String> {
        local_ips()
            .into_iter()
            .map(|ip| format!("http://{ip}:{}/", self.port))
            .collect()
    }

    pub fn stop(self) {
        for _ in &self.workers {
            self.server.unblock();
        }
        for w in self.workers {
            let _ = w.join();
        }
    }
}

/// IPv4-адреси локальної мережі, найімовірніша першою.
///
/// Першою йде адреса, через яку йде маршрут за замовчуванням (справжня
/// Wi-Fi чи Ethernet-мережа). Віртуальні адаптери (WSL, Hyper-V, VirtualBox,
/// VMware, Docker, VPN) — останніми: інший телефон до них не дістанеться.
pub fn local_ips() -> Vec<IpAddr> {
    let ifaces: Vec<(String, IpAddr)> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| !i.is_loopback())
        .map(|i| (i.name.clone(), i.ip()))
        .filter(|(_, ip)| ip.is_ipv4())
        .collect();
    rank_ips(ifaces, default_route_ip())
}

/// Локальна адреса маршруту за замовчуванням. UDP `connect` нічого не
/// надсилає, лише вибирає інтерфейс; без мережі повертає `None`.
fn default_route_ip() -> Option<IpAddr> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    let ip = s.local_addr().ok()?.ip();
    (!ip.is_unspecified() && !ip.is_loopback()).then_some(ip)
}

fn is_virtual_iface(name: &str) -> bool {
    let n = name.to_lowercase();
    [
        "vethernet",
        "wsl",
        "hyper-v",
        "virtualbox",
        "vboxnet",
        "vmware",
        "vmnet",
        "docker",
        "br-",
        "veth",
        "virbr",
        "tailscale",
        "zerotier",
        "wireguard",
        "tun",
        "tap",
        "utun",
        "loopback",
        "npcap",
        "bluetooth",
    ]
    .iter()
    .any(|v| n.contains(v))
}

fn rank_ips(ifaces: Vec<(String, IpAddr)>, default: Option<IpAddr>) -> Vec<IpAddr> {
    let mut v: Vec<(u8, IpAddr)> = ifaces
        .into_iter()
        .filter_map(|(name, ip)| {
            let IpAddr::V4(v4) = ip else { return None };
            if v4.is_link_local() || v4.is_unspecified() {
                return None;
            }
            let o = v4.octets();
            let rank = if Some(ip) == default && !is_virtual_iface(&name) {
                0
            } else if is_virtual_iface(&name) {
                9
            } else if o[0] == 192 && o[1] == 168 {
                1
            } else if o[0] == 10 {
                2
            } else if v4.is_private() {
                3 // 172.16–31: часто віртуальні мережі
            } else {
                5
            };
            Some((rank, ip))
        })
        .collect();
    v.sort_by_key(|(r, _)| *r);
    let mut out: Vec<IpAddr> = Vec::new();
    for (_, ip) in v {
        if !out.contains(&ip) {
            out.push(ip);
        }
    }
    out
}

fn header(k: &str, v: &str) -> Header {
    Header::from_bytes(k.as_bytes(), v.as_bytes()).expect("коректний заголовок")
}

fn respond<R: Read>(req: Request, resp: Response<R>) {
    let _ = req.respond(resp);
}

fn text(req: Request, code: u16, mime: &str, body: String) {
    respond(
        req,
        Response::from_string(body)
            .with_status_code(code)
            .with_header(header("Content-Type", mime)),
    );
}

fn json<T: Serialize>(req: Request, v: &T) {
    text(
        req,
        200,
        "application/json; charset=utf-8",
        serde_json::to_string(v).unwrap_or_else(|_| "null".into()),
    );
}

fn not_found(req: Request) {
    text(req, 404, "text/plain; charset=utf-8", "Не знайдено".into());
}

fn handle(state: &Arc<RwLock<ShareState>>, req: Request) {
    if *req.method() != Method::Get && *req.method() != Method::Head {
        return text(req, 405, "text/plain; charset=utf-8", "Лише читання".into());
    }
    let raw = req.url().to_string();
    let (path, query) = raw.split_once('?').unwrap_or((&raw, ""));
    let path = url_decode(path);
    let st = state.read().unwrap();
    let segs: Vec<&str> = path.trim_start_matches('/').splitn(3, '/').collect();
    match segs.as_slice() {
        [""] => text(req, 200, "text/html; charset=utf-8", index_html(&st)),
        ["api", "search"] => {
            let q = query_param(query, "q").unwrap_or_default();
            json(req, &st.library.search(&q, 30))
        }
        ["api", "info"] => json(
            req,
            &serde_json::json!({
                "app": "uautonomy",
                "core": crate::version(),
                "name": st.device_name,
                "packs": st.store.installed().len(),
            }),
        ),
        ["card", id] => match st.library.card(id) {
            Some(c) => text(req, 200, "text/html; charset=utf-8", card_html(c)),
            None => not_found(req),
        },
        ["article", id] => match st.library.doc(&format!("article:{id}")) {
            Some(d) => text(
                req,
                200,
                "text/html; charset=utf-8",
                page(
                    &d.title,
                    &format!(
                        "<pre class=md>{}</pre><p class=src>Джерело: {}</p>",
                        esc(&d.text),
                        esc(&d.source)
                    ),
                ),
            ),
            None => not_found(req),
        },
        ["map"] => {
            let maps: Vec<MapInfo> = installed_maps(&st).into_iter().map(|(m, _)| m).collect();
            text(
                req,
                200,
                "text/html; charset=utf-8",
                webmap::map_page(&maps),
            )
        }
        ["map", "lib", name] => match webmap::asset(name) {
            Some((mime, data)) => respond(
                req,
                Response::from_data(data)
                    .with_header(header("Content-Type", mime))
                    .with_header(header("Cache-Control", "max-age=604800")),
            ),
            None => not_found(req),
        },
        ["map", "fonts", rest] => {
            let range = rest.rsplit('/').next().unwrap_or("");
            let range = range.strip_suffix(".pbf").unwrap_or(range);
            respond(
                req,
                Response::from_data(webmap::glyphs(range))
                    .with_header(header("Content-Type", "application/x-protobuf"))
                    .with_header(header("Cache-Control", "max-age=604800")),
            )
        }
        ["map", "style", file] => {
            let region = file.strip_suffix(".json").unwrap_or(file);
            let origin = req
                .headers()
                .iter()
                .find(|h| h.field.equiv("Host"))
                .map(|h| format!("http://{}", h.value.as_str()))
                .unwrap_or_default();
            match installed_maps(&st)
                .into_iter()
                .find(|(m, _)| m.region == region)
            {
                Some((m, _)) => text(
                    req,
                    200,
                    "application/json; charset=utf-8",
                    webmap::style_json(&origin, &m),
                ),
                None => not_found(req),
            }
        }
        ["map", "tiles", rest] => serve_tile(&st, req, rest),
        ["zim", pack, rest] => serve_zim(&st, req, pack, rest),
        ["zim", pack] => serve_zim(&st, req, pack, ""),
        ["packs", "catalog.json"] => json(
            req,
            &Catalog {
                base_url: String::new(),
                packs: st.store.installed(),
            },
        ),
        ["packs", id, rel] if is_safe_id(id) && is_safe_rel_path(rel) => {
            match st.store.file_path(id, rel) {
                Some(p) => serve_file(req, p, "application/octet-stream"),
                None => not_found(req),
            }
        }
        ["app", name] if is_safe_rel_path(name) && !name.contains('/') => {
            match st
                .app_files
                .iter()
                .find(|p| p.file_name().is_some_and(|n| n == *name))
            {
                Some(p) => {
                    let mime = if name.ends_with(".apk") {
                        "application/vnd.android.package-archive"
                    } else {
                        "application/octet-stream"
                    };
                    serve_file(req, p.clone(), mime)
                }
                None => not_found(req),
            }
        }
        _ => not_found(req),
    }
}

/// Відкриті карти: відкривати PMTiles на кожен тайл задорого.
fn open_map(path: &std::path::Path) -> Option<Arc<PmTiles>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<PmTiles>>>> = OnceLock::new();
    let mut c = CACHE.get_or_init(Default::default).lock().unwrap();
    if let Some(m) = c.get(path) {
        return Some(m.clone());
    }
    let m = Arc::new(PmTiles::open(path).ok()?);
    c.insert(path.to_path_buf(), m.clone());
    Some(m)
}

/// Встановлені карти областей: опис і шлях до `.pmtiles`.
fn installed_maps(st: &ShareState) -> Vec<(MapInfo, PathBuf)> {
    let mut out = Vec::new();
    for p in st.store.installed() {
        if p.kind != PackKind::Map {
            continue;
        }
        let Some(region) = p.region.clone() else {
            continue;
        };
        let Some(path) = p
            .files
            .iter()
            .find(|f| f.path.ends_with(".pmtiles"))
            .and_then(|f| st.store.file_path(&p.id, &f.path))
        else {
            continue;
        };
        let Some(pm) = open_map(&path) else {
            continue;
        };
        let h = &pm.header;
        let known = crate::regions::REGIONS.iter().find(|r| r.id == region);
        let title = known
            .map(|r| r.title.to_string())
            .unwrap_or_else(|| p.title.clone());
        // tilemaker не записує межі в заголовок PMTiles (нулі): беремо
        // межі області, інакше MapLibre не запитує жодного тайла.
        let bounds = match known {
            Some(r) if !h.has_bounds() => r.bbox,
            _ => h.bounds,
        };
        out.push((
            MapInfo {
                region,
                title,
                min_zoom: h.min_zoom,
                max_zoom: h.max_zoom,
                bounds,
            },
            path,
        ));
    }
    out.sort_by(|a, b| a.0.title.cmp(&b.0.title));
    out
}

/// `/map/tiles/<область>/<z>/<x>/<y>.pbf` → векторний тайл (MVT).
fn serve_tile(st: &ShareState, req: Request, rest: &str) {
    let parts: Vec<&str> = rest.split('/').collect();
    let [region, z, x, y] = parts.as_slice() else {
        return not_found(req);
    };
    let y = y.strip_suffix(".pbf").unwrap_or(y);
    let (Ok(z), Ok(x), Ok(y)) = (z.parse::<u8>(), x.parse::<u32>(), y.parse::<u32>()) else {
        return not_found(req);
    };
    // Швидкий шлях: пакет `map-<область>`; інакше — перебір усіх карт.
    let direct = st.store.get(&format!("map-{region}")).and_then(|p| {
        let f = p.files.iter().find(|f| f.path.ends_with(".pmtiles"))?;
        st.store.file_path(&p.id, &f.path)
    });
    let Some(path) = direct.or_else(|| {
        installed_maps(st)
            .into_iter()
            .find(|(m, _)| m.region == *region)
            .map(|(_, p)| p)
    }) else {
        return not_found(req);
    };
    let Some(pm) = open_map(&path) else {
        return not_found(req);
    };
    match pm.tile(z, x, y) {
        Ok(Some(data)) => respond(
            req,
            Response::from_data(data)
                .with_header(header("Content-Type", "application/x-protobuf"))
                .with_header(header("Cache-Control", "max-age=86400")),
        ),
        // Тайла немає (за межами області): порожня відповідь, не помилка.
        Ok(None) => respond(req, Response::empty(204)),
        Err(_) => not_found(req),
    }
}

fn serve_zim(st: &ShareState, req: Request, pack: &str, rest: &str) {
    let Some(zim) = st.library.zim(pack) else {
        return not_found(req);
    };
    let entry = if rest.is_empty() {
        zim.main_page()
    } else {
        zim.get(rest)
    };
    let Some(e) = entry else {
        return not_found(req);
    };
    if rest.is_empty() {
        let target = format!("/zim/{pack}/{}", url_encode_path(&e.path()));
        return respond(
            req,
            Response::empty(302).with_header(header("Location", &target)),
        );
    }
    match zim.content(&e) {
        Ok((mime, data)) => respond(
            req,
            Response::from_data(data)
                .with_header(header("Content-Type", &mime))
                .with_header(header("Cache-Control", "max-age=86400")),
        ),
        Err(_) => not_found(req),
    }
}

/// Віддає файл з підтримкою `Range`, щоб інший телефон міг докачувати.
fn serve_file(req: Request, path: PathBuf, mime: &str) {
    let Ok(mut f) = fs::File::open(&path) else {
        return not_found(req);
    };
    let size = f.metadata().map(|m| m.len()).unwrap_or(0);
    let range = req
        .headers()
        .iter()
        .find(|h| h.field.equiv("Range"))
        .and_then(|h| parse_range(h.value.as_str(), size));
    let base = vec![
        header("Content-Type", mime),
        header("Accept-Ranges", "bytes"),
    ];
    match range {
        Some((start, end)) => {
            if f.seek(SeekFrom::Start(start)).is_err() {
                return not_found(req);
            }
            let len = end - start + 1;
            let mut headers = base;
            headers.push(header(
                "Content-Range",
                &format!("bytes {start}-{end}/{size}"),
            ));
            respond(
                req,
                Response::new(
                    StatusCode(206),
                    headers,
                    f.take(len),
                    Some(len as usize),
                    None,
                ),
            )
        }
        None => respond(
            req,
            Response::new(StatusCode(200), base, f, Some(size as usize), None),
        ),
    }
}

/// `bytes=N-` або `bytes=N-M` → (початок, кінець включно).
fn parse_range(v: &str, size: u64) -> Option<(u64, u64)> {
    let r = v.trim().strip_prefix("bytes=")?;
    let (a, b) = r.split_once('-')?;
    let start: u64 = a.trim().parse().ok()?;
    let end: u64 = if b.trim().is_empty() {
        size.checked_sub(1)?
    } else {
        b.trim().parse::<u64>().ok()?.min(size.checked_sub(1)?)
    };
    (start <= end).then_some((start, end))
}

fn query_param(q: &str, key: &str) -> Option<String> {
    q.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then(|| url_decode(&v.replace('+', " ")))
    })
}

pub fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = |c: u8| (c as char).to_digit(16);
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn url_encode_path(s: &str) -> String {
    s.split('/').map(url_encode).collect::<Vec<_>>().join("/")
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const CSS: &str = "body{font-family:system-ui,sans-serif;max-width:720px;margin:0 auto;padding:12px;line-height:1.45;background:#fff;color:#111}\
@media(prefers-color-scheme:dark){body{background:#111;color:#eee}a{color:#8cf}}\
input{width:100%;font-size:18px;padding:10px;box-sizing:border-box}\
.warn{background:#fff3cd;color:#5c4400;padding:8px;border-radius:6px}.src{color:#888;font-size:14px}\
li{margin:6px 0}.hit{padding:8px 0;border-bottom:1px solid #8884}pre.md{white-space:pre-wrap;font-family:inherit}";

fn page(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html lang=uk><head><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'>\
<title>{t}</title><style>{CSS}</style></head><body><p><a href='/'>← Автономія</a></p><h1>{t}</h1>{body}\
<p class=warn>У небезпеці телефонуйте 112. Неперевірені фахівцем матеріали позначені.</p></body></html>",
        t = esc(title)
    )
}

fn card_html(c: &crate::Card) -> String {
    let mut b = String::new();
    if !c.reviewed {
        b.push_str("<p class=warn>Ще не перевірено фахівцем.</p>");
    }
    if let Some(s) = &c.summary {
        b.push_str(&format!("<p>{}</p>", esc(s)));
    }
    b.push_str("<ol>");
    for s in &c.steps {
        b.push_str(&format!("<li>{}</li>", esc(s)));
    }
    b.push_str("</ol>");
    if !c.warnings.is_empty() {
        b.push_str("<h3>Чого не робити</h3><ul>");
        for w in &c.warnings {
            b.push_str(&format!("<li>{}</li>", esc(w)));
        }
        b.push_str("</ul>");
    }
    b.push_str(&format!("<p class=src>Джерело: {}</p>", esc(&c.source)));
    page(&c.title, &b)
}

fn index_html(st: &ShareState) -> String {
    let mut cards = String::new();
    for cat in crate::Category::ALL {
        let items: Vec<_> = st
            .library
            .cards()
            .iter()
            .filter(|c| c.category == cat)
            .collect();
        if items.is_empty() {
            continue;
        }
        cards.push_str(&format!("<h3>{}</h3><ul>", cat.title_uk()));
        for c in items {
            cards.push_str(&format!(
                "<li><a href='/card/{}'>{}</a></li>",
                url_encode(&c.id),
                esc(&c.title)
            ));
        }
        cards.push_str("</ul>");
    }
    let mut zims = String::new();
    for (pack, z) in st.library.zims() {
        let title = z.metadata("Title").unwrap_or_else(|| pack.to_string());
        zims.push_str(&format!(
            "<li><a href='/zim/{}'>{}</a></li>",
            url_encode(pack),
            esc(&title)
        ));
    }
    let mut packs = String::new();
    for p in st.store.installed() {
        packs.push_str(&format!(
            "<li>{} — {:.1} МБ</li>",
            esc(&p.title),
            p.size_bytes() as f64 / 1_048_576.0
        ));
    }
    let mut apps = String::new();
    for f in &st.app_files {
        if let Some(n) = f.file_name() {
            let n = n.to_string_lossy();
            apps.push_str(&format!(
                "<li><a href='/app/{}'>{}</a></li>",
                url_encode(&n),
                esc(&n)
            ));
        }
    }
    let script = "<script>const q=document.getElementById('q'),r=document.getElementById('r');let t;\
q.oninput=()=>{clearTimeout(t);t=setTimeout(async()=>{if(!q.value.trim()){r.innerHTML='';return}\
const hs=await (await fetch('/api/search?q='+encodeURIComponent(q.value))).json();\
r.innerHTML=hs.map(h=>{const href=h.kind==='card'?'/card/'+encodeURIComponent(h.key.slice(5)):h.kind==='article'?'/article/'+encodeURIComponent(h.key.slice(8)):'/zim/'+h.key.slice(4).split('/').map(encodeURIComponent).join('/');\
const e=s=>s.replace(/[&<>]/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;'}[c]));\
return `<div class=hit><a href='${href}'>${e(h.title)}</a><br><small>${e(h.snippet||'')}</small> <span class=src>${e(h.source)}${h.reviewed?'':' · не перевірено'}</span></div>`}).join('')||'<p>Нічого не знайдено</p>'},200)}</script>";
    format!(
        "<!doctype html><html lang=uk><head><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'>\
<title>Автономія</title><style>{CSS}</style></head><body><h1>Автономія</h1><p>Офлайн-бібліотека з пристрою «{name}». Працює без інтернету.</p>\
<p class=warn>У небезпеці телефонуйте 112. Неперевірені фахівцем матеріали позначені.</p>\
<input id=q placeholder='Пошук: кровотеча, вода, укриття…' autofocus><div id=r></div>{maps_h}\
<h2>Термінові картки</h2>{cards}{zim_h}{apps_h}<h2>Пакети на цьому пристрої</h2><ul>{packs}</ul>\
<p class=src>У застосунку «Автономія»: вкладка «Пакети» → поле «Адреса каталогу або іншого пристрою» → вставте цю адресу і натисніть стрілку.</p>{script}</body></html>",
        name = esc(&st.device_name),
        maps_h = if st.store.installed().iter().any(|p| p.kind == PackKind::Map) {
            "<p><a href='/map'><b>🗺 Офлайн-карти</b></a></p>"
        } else {
            ""
        },
        zim_h = if zims.is_empty() { String::new() } else { format!("<h2>Довідники</h2><ul>{zims}</ul>") },
        apps_h = if apps.is_empty() { String::new() } else { format!("<h2>Встановити застосунок</h2><ul>{apps}</ul>") },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Card, Category};

    fn get(url: &str, range: Option<&str>) -> (u16, Vec<u8>) {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build()
            .into();
        let mut r = agent.get(url);
        if let Some(v) = range {
            r = r.header("Range", v);
        }
        let mut resp = r.call().unwrap();
        let code = resp.status().as_u16();
        (code, resp.body_mut().read_to_vec().unwrap())
    }

    #[test]
    fn serves_library() {
        let t = tempfile::tempdir().unwrap();
        let store = crate::store::tests::test_store(t.path());
        let src = t.path().join("src");
        crate::store::tests::sample_pack(&src, "pocket");
        store.install_from_dir(&src, &mut |_, _| {}).unwrap();
        let lib = Library::new(vec![Card::simple(
            "c1",
            Category::FirstAid,
            "Опік <b>",
            &["Охолодіть"],
            "т",
        )]);
        let state = Arc::new(RwLock::new(ShareState::new(store, lib)));
        let s = ShareServer::start(state, 0).unwrap();
        let base = format!("http://127.0.0.1:{}", s.port());

        let (c, body) = get(&format!("{base}/"), None);
        assert_eq!(c, 200);
        let html = String::from_utf8(body).unwrap();
        assert!(html.contains("Опік &lt;b&gt;"));
        let (_, body) = get(
            &format!("{base}/api/search?q=%D0%BE%D0%BF%D1%96%D0%BA"),
            None,
        );
        assert!(String::from_utf8(body).unwrap().contains("card:c1"));
        let (c, _) = get(&format!("{base}/card/c1"), None);
        assert_eq!(c, 200);
        let (c, body) = get(
            &format!("{base}/packs/pocket/cards.json"),
            Some("bytes=2-5"),
        );
        assert_eq!(c, 206);
        assert_eq!(body.len(), 4);
        let (c, _) = get(&format!("{base}/packs/pocket/..%2F..%2Fetc"), None);
        assert_eq!(c, 404);
        let (c, _) = get(&format!("{base}/packs/pocket/manifest.json"), None);
        assert_eq!(c, 200);
        s.stop();
    }

    #[test]
    fn serves_map_for_browsers() {
        let t = tempfile::tempdir().unwrap();
        let store = crate::store::tests::test_store(&t.path().join("store"));
        let src = t.path().join("map-kyiv");
        fs::create_dir_all(&src).unwrap();
        let tile = b"\x1a\x00mvt".to_vec();
        fs::write(
            src.join("kyiv.pmtiles"),
            crate::pmtiles::write_test_pmtiles(&[((10, 598, 345), tile.clone())], true),
        )
        .unwrap();
        let m = crate::store::manifest_for_dir(
            &src,
            crate::PackManifest {
                id: "map-kyiv".into(),
                tier: crate::PackTier::Family,
                kind: PackKind::Map,
                title: "Карта: Київ".into(),
                description: String::new(),
                version: "1".into(),
                files: vec![],
                content_license: "ODbL-1.0".into(),
                region: Some("kyiv".into()),
                model_id: None,
                signature: None,
            },
        )
        .unwrap();
        fs::write(src.join("manifest.json"), m.to_json()).unwrap();
        crate::store::tests::sign_dir(&src, 1000);
        store.install_from_dir(&src, &mut |_, _| {}).unwrap();
        let state = Arc::new(RwLock::new(ShareState::new(store, Library::new(vec![]))));
        let s = ShareServer::start(state, 0).unwrap();
        let base = format!("http://127.0.0.1:{}", s.port());

        let (_, body) = get(&format!("{base}/"), None);
        assert!(String::from_utf8(body).unwrap().contains("/map"));
        let (c, body) = get(&format!("{base}/map"), None);
        assert_eq!(c, 200);
        assert!(String::from_utf8(body).unwrap().contains("value='kyiv'"));
        let (c, body) = get(&format!("{base}/map/style/kyiv.json"), None);
        assert_eq!(c, 200);
        let style: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(style["sources"]["omt"]["tiles"][0]
            .as_str()
            .unwrap()
            .starts_with("http://127.0.0.1:"));
        let (c, body) = get(&format!("{base}/map/tiles/kyiv/10/598/345.pbf"), None);
        assert_eq!((c, body), (200, tile));
        let (c, _) = get(&format!("{base}/map/tiles/kyiv/10/1/1.pbf"), None);
        assert_eq!(c, 204);
        let (c, _) = get(&format!("{base}/map/tiles/lvivska/10/1/1.pbf"), None);
        assert_eq!(c, 404);
        let (c, body) = get(
            &format!("{base}/map/fonts/Noto%20Sans%20Regular/1024-1279.pbf"),
            None,
        );
        assert_eq!(c, 200);
        assert!(body.len() > 10_000);
        let (c, body) = get(&format!("{base}/map/lib/maplibre-gl.js"), None);
        assert_eq!(c, 200);
        assert!(body.len() > 500_000);
        s.stop();
    }

    #[test]
    fn ranges_and_encoding() {
        assert_eq!(parse_range("bytes=0-", 10), Some((0, 9)));
        assert_eq!(parse_range("bytes=5-100", 10), Some((5, 9)));
        assert_eq!(parse_range("bytes=11-", 10), None);
        assert_eq!(url_decode(&url_encode("Вода й сіль/1")), "Вода й сіль/1");
        assert_eq!(query_param("a=1&q=%D0%B2+x", "q").as_deref(), Some("в x"));
    }

    #[test]
    fn lan_ip_beats_virtual_adapter() {
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        let ifaces = vec![
            (
                "vEthernet (WSL (Hyper-V firewall))".to_string(),
                ip("172.26.192.1"),
            ),
            ("Ethernet".to_string(), ip("10.10.10.32")),
            ("Ethernet 2".to_string(), ip("169.254.10.1")),
        ];
        assert_eq!(
            rank_ips(ifaces.clone(), None),
            vec![ip("10.10.10.32"), ip("172.26.192.1")]
        );
        assert_eq!(
            rank_ips(ifaces, Some(ip("10.10.10.32")))[0],
            ip("10.10.10.32")
        );
        let hotspot = vec![
            ("wlan0".to_string(), ip("172.20.10.2")),
            ("ap0".to_string(), ip("192.168.43.1")),
        ];
        assert_eq!(rank_ips(hotspot, None)[0], ip("192.168.43.1"));
    }
}
