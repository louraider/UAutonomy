//! Області України для пакетів офлайн-карт. Межі приблизні (з запасом),
//! точне вирізання робить інструмент збирання пакетів карт.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Region {
    pub id: &'static str,
    pub title: &'static str,
    /// [захід, південь, схід, північ] у градусах.
    pub bbox: [f64; 4],
}

impl Region {
    pub fn center(&self) -> (f64, f64) {
        (
            (self.bbox[1] + self.bbox[3]) / 2.0,
            (self.bbox[0] + self.bbox[2]) / 2.0,
        )
    }
    /// Ідентифікатор пакета карти цієї області.
    pub fn pack_id(&self) -> String {
        format!("map-{}", self.id)
    }
}

pub const UKRAINE_BBOX: [f64; 4] = [22.1, 44.3, 40.3, 52.4];

pub const REGIONS: &[Region] = &[
    Region {
        id: "vinnytska",
        title: "Вінницька",
        bbox: [27.3, 47.9, 29.9, 49.9],
    },
    Region {
        id: "volynska",
        title: "Волинська",
        bbox: [23.5, 50.2, 26.1, 52.0],
    },
    Region {
        id: "dnipropetrovska",
        title: "Дніпропетровська",
        bbox: [32.9, 47.4, 36.4, 49.2],
    },
    Region {
        id: "donetska",
        title: "Донецька",
        bbox: [36.5, 46.9, 38.9, 49.3],
    },
    Region {
        id: "zhytomyrska",
        title: "Житомирська",
        bbox: [27.1, 49.6, 29.6, 51.6],
    },
    Region {
        id: "zakarpatska",
        title: "Закарпатська",
        bbox: [22.1, 47.9, 24.7, 49.1],
    },
    Region {
        id: "zaporizka",
        title: "Запорізька",
        bbox: [34.1, 46.2, 37.3, 48.2],
    },
    Region {
        id: "ivano-frankivska",
        title: "Івано-Франківська",
        bbox: [23.5, 47.7, 25.8, 49.4],
    },
    Region {
        id: "kyivska",
        title: "Київська",
        bbox: [29.2, 49.2, 32.2, 51.6],
    },
    Region {
        id: "kyiv",
        title: "Київ",
        bbox: [30.2, 50.2, 30.9, 50.6],
    },
    Region {
        id: "kirovohradska",
        title: "Кіровоградська",
        bbox: [30.8, 47.7, 33.9, 49.2],
    },
    Region {
        id: "luhanska",
        title: "Луганська",
        bbox: [37.9, 47.8, 40.3, 50.1],
    },
    Region {
        id: "lvivska",
        title: "Львівська",
        bbox: [22.6, 49.0, 25.0, 50.9],
    },
    Region {
        id: "mykolaivska",
        title: "Миколаївська",
        bbox: [30.2, 46.3, 33.2, 48.2],
    },
    Region {
        id: "odeska",
        title: "Одеська",
        bbox: [28.2, 45.2, 31.1, 48.3],
    },
    Region {
        id: "poltavska",
        title: "Полтавська",
        bbox: [32.0, 48.7, 35.5, 50.6],
    },
    Region {
        id: "rivnenska",
        title: "Рівненська",
        bbox: [25.2, 50.0, 27.7, 52.0],
    },
    Region {
        id: "sumska",
        title: "Сумська",
        bbox: [32.9, 50.1, 35.7, 52.4],
    },
    Region {
        id: "ternopilska",
        title: "Тернопільська",
        bbox: [24.9, 48.5, 26.7, 50.3],
    },
    Region {
        id: "kharkivska",
        title: "Харківська",
        bbox: [34.8, 48.6, 38.1, 50.5],
    },
    Region {
        id: "khersonska",
        title: "Херсонська",
        bbox: [31.4, 45.9, 35.0, 47.6],
    },
    Region {
        id: "khmelnytska",
        title: "Хмельницька",
        bbox: [26.0, 48.4, 27.9, 50.6],
    },
    Region {
        id: "cherkaska",
        title: "Черкаська",
        bbox: [29.6, 48.4, 32.9, 50.2],
    },
    Region {
        id: "chernivetska",
        title: "Чернівецька",
        bbox: [24.9, 47.7, 27.5, 48.7],
    },
    Region {
        id: "chernihivska",
        title: "Чернігівська",
        bbox: [30.5, 50.6, 33.6, 52.4],
    },
    Region {
        id: "krym",
        title: "Автономна Республіка Крим",
        bbox: [32.4, 44.3, 36.7, 46.2],
    },
];

pub fn by_id(id: &str) -> Option<&'static Region> {
    REGIONS.iter().find(|r| r.id == id)
}

/// Область за координатами (перша, в межі якої потрапляє точка; Київ має
/// пріоритет над Київською).
pub fn at(lat: f64, lon: f64) -> Option<&'static Region> {
    let inside =
        |r: &&Region| lon >= r.bbox[0] && lon <= r.bbox[2] && lat >= r.bbox[1] && lat <= r.bbox[3];
    by_id("kyiv")
        .filter(inside)
        .or_else(|| REGIONS.iter().find(inside))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_are_sane() {
        assert_eq!(REGIONS.len(), 26);
        for r in REGIONS {
            assert!(r.bbox[0] < r.bbox[2] && r.bbox[1] < r.bbox[3], "{}", r.id);
            assert!(
                r.bbox[0] >= UKRAINE_BBOX[0] && r.bbox[3] <= UKRAINE_BBOX[3],
                "{}",
                r.id
            );
            assert!(crate::pack::is_safe_id(&r.pack_id()));
        }
        assert_eq!(at(50.45, 30.52).unwrap().id, "kyiv");
        assert_eq!(at(49.84, 24.03).unwrap().id, "lvivska");
    }
}
