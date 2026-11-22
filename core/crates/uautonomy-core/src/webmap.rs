//! Карти в браузері для пристроїв без застосунку (iPhone, ноутбук): сервер
//! Wi-Fi-обміну віддає сторінку з MapLibre GL JS, стиль OpenMapTiles,
//! векторні тайли з PMTiles і гліфи шрифту. Усе вбудовано в програму, тож
//! інтернет не потрібен ні роздавачу, ні тому, хто дивиться.
//!
//! Сторонні файли в `assets/web`: MapLibre GL JS 5.24.1 (BSD-3-Clause,
//! `maplibre-LICENSE.txt`), гліфи Noto Sans Regular з protomaps/basemaps-assets
//! (SIL OFL 1.1, `fonts/OFL.txt`).

/// Вбудований файл сторінки карти: (MIME, вміст).
pub fn asset(name: &str) -> Option<(&'static str, &'static [u8])> {
    Some(match name {
        "maplibre-gl.js" => (
            "application/javascript; charset=utf-8",
            include_bytes!("../assets/web/maplibre-gl.js"),
        ),
        "maplibre-gl.css" => (
            "text/css; charset=utf-8",
            include_bytes!("../assets/web/maplibre-gl.css"),
        ),
        "LICENSE.txt" => (
            "text/plain; charset=utf-8",
            include_bytes!("../assets/web/maplibre-LICENSE.txt"),
        ),
        _ => return None,
    })
}

pub const FONT: &str = "Noto Sans Regular";

/// Гліфи для діапазону `start-end`. Для діапазонів, яких немає (ієрогліфи
/// тощо), — порожній, але коректний набір: підписи просто без цих символів.
pub fn glyphs(range: &str) -> &'static [u8] {
    match range {
        "0-255" => include_bytes!("../assets/web/fonts/0-255.pbf"),
        "256-511" => include_bytes!("../assets/web/fonts/256-511.pbf"),
        "1024-1279" => include_bytes!("../assets/web/fonts/1024-1279.pbf"),
        "8192-8447" => include_bytes!("../assets/web/fonts/8192-8447.pbf"),
        _ => &[],
    }
}

/// Карта однієї області для сторінки й стилю.
#[derive(Debug, Clone)]
pub struct MapInfo {
    pub region: String,
    pub title: String,
    pub min_zoom: u8,
    pub max_zoom: u8,
    /// [захід, південь, схід, північ]
    pub bounds: [f64; 4],
}

/// Стиль MapLibre для схеми OpenMapTiles (так збирає tilemaker).
/// `origin` — `http://адреса:порт`: MapLibre потребує абсолютних адрес.
pub fn style_json(origin: &str, m: &MapInfo) -> String {
    // tilemaker кладе назву в `name:latin` (у вихідному письмі, тобто кирилицею).
    let name = serde_json::json!([
        "coalesce",
        ["get", "name:uk"],
        ["get", "name"],
        ["get", "name:latin"]
    ]);
    let font = serde_json::json!([FONT]);
    let line = |id: &str,
                layer: &str,
                filter: serde_json::Value,
                color: &str,
                w: [f64; 4],
                min: u8| {
        serde_json::json!({
            "id": id, "type": "line", "source": "omt", "source-layer": layer,
            "minzoom": min, "filter": filter,
            "layout": {"line-cap": "round", "line-join": "round"},
            "paint": {"line-color": color,
                "line-width": ["interpolate", ["exponential", 1.5], ["zoom"], w[0], w[1], w[2], w[3]]}
        })
    };
    let class_in = |classes: &[&str]| {
        let mut v = vec![
            serde_json::json!("match"),
            serde_json::json!(["get", "class"]),
        ];
        v.push(serde_json::json!(classes));
        v.push(serde_json::json!(true));
        v.push(serde_json::json!(false));
        serde_json::Value::Array(v)
    };
    let layers = serde_json::json!([
        {"id": "background", "type": "background", "paint": {"background-color": "#f6f3ee"}},
        {"id": "landcover-wood", "type": "fill", "source": "omt", "source-layer": "landcover",
         "filter": class_in(&["wood", "forest"]), "paint": {"fill-color": "#cfe3c2"}},
        {"id": "landcover-grass", "type": "fill", "source": "omt", "source-layer": "landcover",
         "filter": class_in(&["grass", "farmland", "meadow", "scrub"]), "paint": {"fill-color": "#e6eedb"}},
        {"id": "landuse-residential", "type": "fill", "source": "omt", "source-layer": "landuse",
         "filter": class_in(&["residential", "suburb", "neighbourhood"]), "paint": {"fill-color": "#ece7e1"}},
        {"id": "landuse-hospital", "type": "fill", "source": "omt", "source-layer": "landuse",
         "filter": class_in(&["hospital"]), "paint": {"fill-color": "#f7dcdc"}},
        {"id": "park", "type": "fill", "source": "omt", "source-layer": "park",
         "paint": {"fill-color": "#d4e8c6", "fill-opacity": 0.8}},
        {"id": "water", "type": "fill", "source": "omt", "source-layer": "water",
         "paint": {"fill-color": "#a9cbe8"}},
        line("waterway", "waterway", serde_json::json!(true), "#a9cbe8", [8.0, 0.5, 18.0, 6.0], 8),
        {"id": "building", "type": "fill", "source": "omt", "source-layer": "building", "minzoom": 13,
         "paint": {"fill-color": "#ddd6cc", "fill-outline-color": "#cbc2b6"}},
        {"id": "boundary", "type": "line", "source": "omt", "source-layer": "boundary",
         "filter": ["<=", ["get", "admin_level"], 6],
         "paint": {"line-color": "#9e8fb0", "line-dasharray": [3, 2],
                   "line-width": ["interpolate", ["linear"], ["zoom"], 4, 0.6, 12, 1.6]}},
        line("road-path", "transportation", class_in(&["path", "track"]), "#b9a58f", [13.0, 0.5, 18.0, 2.0], 13),
        line("road-minor", "transportation", class_in(&["minor", "service"]), "#ffffff", [12.0, 0.5, 18.0, 10.0], 12),
        line("road-tertiary", "transportation", class_in(&["tertiary"]), "#ffffff", [10.0, 0.6, 18.0, 14.0], 10),
        line("road-secondary", "transportation", class_in(&["secondary"]), "#f7e7a6", [8.0, 0.6, 18.0, 16.0], 8),
        line("road-primary", "transportation", class_in(&["primary", "trunk"]), "#f5c97a", [6.0, 0.6, 18.0, 18.0], 6),
        line("road-motorway", "transportation", class_in(&["motorway"]), "#e8a15c", [5.0, 0.8, 18.0, 20.0], 5),
        line("rail", "transportation", class_in(&["rail", "transit"]), "#9a9a9a", [10.0, 0.5, 18.0, 3.0], 10),
        {"id": "road-label", "type": "symbol", "source": "omt", "source-layer": "transportation_name",
         "minzoom": 13,
         "layout": {"symbol-placement": "line", "text-field": name, "text-font": font,
                    "text-size": ["interpolate", ["linear"], ["zoom"], 13, 10, 18, 14]},
         "paint": {"text-color": "#555", "text-halo-color": "#fff", "text-halo-width": 1.5}},
        {"id": "water-label", "type": "symbol", "source": "omt", "source-layer": "water_name",
         "layout": {"text-field": name, "text-font": font, "text-size": 12},
         "paint": {"text-color": "#3d6f99", "text-halo-color": "#fff", "text-halo-width": 1}},
        {"id": "poi-label", "type": "symbol", "source": "omt", "source-layer": "poi", "minzoom": 16,
         "layout": {"text-field": name, "text-font": font, "text-size": 11, "text-max-width": 8},
         "paint": {"text-color": "#6b5b4b", "text-halo-color": "#fff", "text-halo-width": 1.2}},
        {"id": "housenumber", "type": "symbol", "source": "omt", "source-layer": "housenumber", "minzoom": 17,
         "layout": {"text-field": ["get", "housenumber"], "text-font": font, "text-size": 10},
         "paint": {"text-color": "#8a8077"}},
        {"id": "place-minor", "type": "symbol", "source": "omt", "source-layer": "place",
         "filter": class_in(&["village", "hamlet", "suburb", "quarter", "neighbourhood"]), "minzoom": 10,
         "layout": {"text-field": name, "text-font": font, "text-size": 12},
         "paint": {"text-color": "#333", "text-halo-color": "#fff", "text-halo-width": 1.5}},
        {"id": "place-major", "type": "symbol", "source": "omt", "source-layer": "place",
         "filter": class_in(&["city", "town"]),
         "layout": {"text-field": name, "text-font": font,
                    "text-size": ["interpolate", ["linear"], ["zoom"], 6, 12, 14, 20]},
         "paint": {"text-color": "#111", "text-halo-color": "#fff", "text-halo-width": 2}}
    ]);
    serde_json::json!({
        "version": 8,
        "name": format!("Автономія: {}", m.title),
        "glyphs": format!("{origin}/map/fonts/{{fontstack}}/{{range}}.pbf"),
        "sources": {"omt": {
            "type": "vector",
            "tiles": [format!("{origin}/map/tiles/{}/{{z}}/{{x}}/{{y}}.pbf", m.region)],
            "minzoom": m.min_zoom, "maxzoom": m.max_zoom, "bounds": m.bounds,
            "attribution": "© учасники OpenStreetMap"
        }},
        "layers": layers
    })
    .to_string()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Сторінка карти з вибором області.
pub fn map_page(maps: &[MapInfo]) -> String {
    if maps.is_empty() {
        return "<!doctype html><html lang=uk><head><meta charset=utf-8>\
<meta name=viewport content='width=device-width,initial-scale=1'><title>Карти</title></head>\
<body style='font-family:system-ui;padding:16px'><p><a href='/'>← Автономія</a></p>\
<p>На цьому пристрої ще немає жодної карти. Завантажте карту області в застосунку «Автономія» (Пакети).</p></body></html>"
            .into();
    }
    let options: String = maps
        .iter()
        .map(|m| {
            let b = m.bounds;
            format!(
                "<option value='{}' data-b='{},{},{},{}' data-min='{}'>{}</option>",
                esc(&m.region),
                b[0],
                b[1],
                b[2],
                b[3],
                m.min_zoom,
                esc(&m.title)
            )
        })
        .collect();
    format!(
        "<!doctype html><html lang=uk><head><meta charset=utf-8>\
<meta name=viewport content='width=device-width,initial-scale=1,maximum-scale=1'>\
<title>Карти — Автономія</title><link rel=stylesheet href='/map/lib/maplibre-gl.css'>\
<style>html,body{{margin:0;height:100%;font-family:system-ui,sans-serif}}#m{{position:absolute;top:44px;bottom:0;left:0;right:0}}\
#bar{{height:44px;display:flex;gap:8px;align-items:center;padding:0 8px;background:#0057b7;color:#fff}}\
#bar a{{color:#fff;text-decoration:none}}#bar select{{flex:1;font-size:16px;padding:4px}}</style></head>\
<body><div id=bar><a href='/'>← Автономія</a><select id=r>{options}</select></div><div id=m></div>\
<script src='/map/lib/maplibre-gl.js'></script><script>\
const sel=document.getElementById('r');let map;\
function show(){{const o=sel.selectedOptions[0];const b=o.dataset.b.split(',').map(Number);\
const style=location.origin+'/map/style/'+encodeURIComponent(sel.value)+'.json';\
if(map){{map.setStyle(style);map.fitBounds([[b[0],b[1]],[b[2],b[3]]],{{animate:false}});return}}\
map=new maplibregl.Map({{container:'m',style,bounds:[[b[0],b[1]],[b[2],b[3]]],maxZoom:18,attributionControl:{{compact:true}}}});\
map.addControl(new maplibregl.NavigationControl());map.addControl(new maplibregl.ScaleControl());\
try{{localStorage.setItem('region',sel.value)}}catch(e){{}}}}\
try{{const r=localStorage.getItem('region');if(r&&[...sel.options].some(o=>o.value===r))sel.value=r}}catch(e){{}}\
sel.onchange=()=>{{show();try{{localStorage.setItem('region',sel.value)}}catch(e){{}}}};show();\
</script></body></html>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_is_valid_json_with_absolute_urls() {
        let m = MapInfo {
            region: "kyiv".into(),
            title: "Київ".into(),
            min_zoom: 0,
            max_zoom: 14,
            bounds: [30.2, 50.2, 30.8, 50.6],
        };
        let v: serde_json::Value =
            serde_json::from_str(&style_json("http://10.0.0.2:8642", &m)).unwrap();
        assert_eq!(v["version"], 8);
        assert_eq!(
            v["sources"]["omt"]["tiles"][0],
            "http://10.0.0.2:8642/map/tiles/kyiv/{z}/{x}/{y}.pbf"
        );
        assert!(v["glyphs"]
            .as_str()
            .unwrap()
            .contains("{fontstack}/{range}"));
        assert!(v["layers"].as_array().unwrap().len() > 15);
        assert!(map_page(&[m]).contains("maplibre-gl.js"));
        assert!(asset("maplibre-gl.js").unwrap().1.len() > 500_000);
        assert!(!glyphs("1024-1279").is_empty(), "кирилиця");
        assert!(glyphs("19968-20223").is_empty());
    }
}
