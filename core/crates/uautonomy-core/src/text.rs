//! Робота з українським текстом: нормалізація, грубий стемінг, токени,
//! перетворення HTML у простий текст, нарізання на фрагменти для ШІ.

/// Закінчення, які відрізаємо (від довших до коротших). Це не повний
/// морфологічний аналіз, а дешевий спосіб знаходити «кровотеча»
/// за запитом «кровотечі» без словників.
const SUFFIXES: &[&str] = &[
    "ування",
    "ювання",
    "ністю",
    "ання",
    "ення",
    "иння",
    "ість",
    "ості",
    "ують",
    "юють",
    "ами",
    "ями",
    "ові",
    "еві",
    "єві",
    "ому",
    "ого",
    "ими",
    "іми",
    "іший",
    "ати",
    "яти",
    "ити",
    "іти",
    "ала",
    "ило",
    "али",
    "или",
    "ував",
    "ій",
    "ою",
    "ею",
    "єю",
    "ія",
    "ії",
    "ію",
    "ам",
    "ям",
    "ах",
    "ях",
    "ом",
    "ем",
    "ти",
    "ть",
    "ла",
    "ло",
    "ли",
    "ив",
    "ує",
    "ює",
    "ий",
    "ий",
    "ої",
    "і",
    "и",
    "а",
    "я",
    "у",
    "ю",
    "е",
    "о",
    "ь",
    "й",
    "є",
    "ї",
];

/// Слова, які нічого не додають до пошуку.
const STOP_WORDS: &[&str] = &[
    "і",
    "й",
    "та",
    "або",
    "чи",
    "в",
    "у",
    "на",
    "з",
    "із",
    "зі",
    "до",
    "від",
    "за",
    "що",
    "як",
    "це",
    "не",
    "по",
    "для",
    "при",
    "про",
    "а",
    "але",
    "же",
    "б",
    "би",
    "то",
    "так",
    "якщо",
    "коли",
    "мені",
    "мене",
    "я",
    "ми",
    "ви",
    "він",
    "вона",
    "воно",
    "вони",
    "його",
    "її",
    "їх",
    "робити",
    "треба",
    "потрібно",
    "можна",
    "чому",
    "де",
];

/// Нижній регістр і уніфікація апострофів та схожих літер.
pub fn normalize(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .filter_map(|c| match c {
            '\'' | '’' | 'ʼ' | '`' | '\u{301}' => None,
            'ґ' => Some('г'),
            'ё' => Some('е'),
            c => Some(c),
        })
        .collect()
}

/// Грубий стемінг одного вже нормалізованого слова.
pub fn stem(word: &str) -> String {
    let n = word.chars().count();
    if n <= 3 {
        return word.to_string();
    }
    for suf in SUFFIXES {
        if let Some(base) = word.strip_suffix(suf) {
            if base.chars().count() >= 3 {
                return base.to_string();
            }
        }
    }
    word.to_string()
}

/// Розбиває текст на основи слів без стоп-слів.
pub fn tokens(s: &str) -> Vec<String> {
    normalize(s)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !STOP_WORDS.contains(w))
        .map(stem)
        .collect()
}

/// Дуже простий HTML → текст: прибирає теги, `<script>`/`<style>`,
/// розкодовує поширені сутності, зберігає абзаци.
pub fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut i = 0;
    let bytes = html.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let rest = &html[i..];
            let lower: String = rest.chars().take(8).collect::<String>().to_lowercase();
            for skip in ["script", "style"] {
                if lower.starts_with(&format!("<{skip}")) {
                    let close = format!("</{skip}");
                    if let Some(p) = rest.to_lowercase().find(&close) {
                        i += p;
                    }
                }
            }
            let end = html[i..]
                .find('>')
                .map(|p| i + p + 1)
                .unwrap_or(bytes.len());
            let tag = html[i..end].to_lowercase();
            if tag.starts_with("<p")
                || tag.starts_with("<br")
                || tag.starts_with("<li")
                || tag.starts_with("<h")
                || tag.starts_with("<div")
                || tag.starts_with("<tr")
                || tag.starts_with("</p")
                || tag.starts_with("</h")
            {
                out.push('\n');
            }
            i = end;
        } else {
            let next = html[i..].find('<').map(|p| i + p).unwrap_or(bytes.len());
            out.push_str(&decode_entities(&html[i..next]));
            i = next;
        }
    }
    // Схлопуємо пробіли й порожні рядки.
    let mut clean = String::with_capacity(out.len());
    for line in out.lines() {
        let l = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if !l.is_empty() {
            clean.push_str(&l);
            clean.push('\n');
        }
    }
    clean
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        rest = &rest[p..];
        let Some(semi) = rest.as_bytes().iter().take(12).position(|b| *b == b';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..semi];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            "mdash" => Some('—'),
            "ndash" => Some('–'),
            "laquo" => Some('«'),
            "raquo" => Some('»'),
            e if e.starts_with("#x") => u32::from_str_radix(&e[2..], 16)
                .ok()
                .and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Ріже текст на фрагменти приблизно по `max_chars` символів, по межах абзаців.
pub fn chunks(text: &str, max_chars: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for para in text.split('\n').map(str::trim).filter(|p| !p.is_empty()) {
        if !cur.is_empty() && cur.chars().count() + para.chars().count() > max_chars {
            out.push(std::mem::take(&mut cur));
        }
        if para.chars().count() > max_chars {
            // Дуже довгий абзац ріжемо по реченнях.
            for sentence in para.split_inclusive(['.', '!', '?']) {
                if !cur.is_empty() && cur.chars().count() + sentence.chars().count() > max_chars {
                    out.push(std::mem::take(&mut cur));
                }
                cur.push_str(sentence);
            }
            continue;
        }
        if !cur.is_empty() {
            cur.push('\n');
        }
        cur.push_str(para);
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stems_match_inflections() {
        assert_eq!(tokens("кровотеча"), tokens("кровотечі"));
        assert_eq!(tokens("Турнікет"), tokens("турнікета"));
        assert_eq!(tokens("п'ять"), tokens("пʼять"));
        assert!(tokens("що робити при опіку").contains(&stem("опіку")));
        assert!(!tokens("що робити").iter().any(|t| t == "що"));
    }

    #[test]
    fn html_is_cleaned() {
        let t = html_to_text(
            "<html><head><style>p{}</style></head><body><h1>Вода</h1><p>Кип&#39;ятіть&nbsp;1&nbsp;хв &amp; більше</p><script>x()</script></body></html>",
        );
        assert_eq!(t, "Вода\nКип'ятіть 1 хв & більше\n");
    }

    #[test]
    fn chunks_respect_limit() {
        let text = "Перший абзац.\nДругий абзац.\nТретій.";
        let c = chunks(text, 15);
        assert_eq!(c.len(), 3);
        assert_eq!(chunks(text, 1000).len(), 1);
    }
}
