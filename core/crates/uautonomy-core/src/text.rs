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
            for skip in ["script", "style", "head"] {
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
                || tag.starts_with("<figcaption")
                || tag.starts_with("<caption")
            {
                out.push('\n');
            } else if tag.starts_with("<td") || tag.starts_with("<th") && !tag.starts_with("<thead")
            {
                // Клітинки таблиць (інфобокс) не злипаються: «Спеціальність: …».
                if !out.ends_with(['\n', ' ']) && !out.is_empty() {
                    out.push_str(" · ");
                }
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

/// Прибирає HTML-коментарі `<!-- … -->`: у статтях там редакторські
/// примітки, які читач бачити не повинен.
pub fn strip_comments(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut rest = md;
    while let Some(p) = rest.find("<!--") {
        out.push_str(&rest[..p]);
        match rest[p..].find("-->") {
            Some(e) => rest = &rest[p + e + 3..],
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

fn is_table_separator(line: &str) -> bool {
    let l = line.trim();
    l.starts_with('|') && l.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn table_cells(line: &str) -> Vec<String> {
    let l = line.trim().trim_start_matches('|').trim_end_matches('|');
    l.split('|').map(|c| c.trim().to_string()).collect()
}

/// У клітинках таблиць замінює ідентифікатори карток (`fa-burns`) на
/// посилання з назвою картки: `[Термічні опіки](card:fa-burns)`.
/// `title` повертає назву картки за id або `None`, якщо такої немає.
pub fn link_card_ids(md: &str, title: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(md.len());
    for line in md.lines() {
        if line.trim_start().starts_with('|') && !is_table_separator(line) {
            let cells: Vec<String> = table_cells(line)
                .into_iter()
                .map(|cell| {
                    let ids: Vec<&str> = cell.split(',').map(str::trim).collect();
                    let titles: Option<Vec<String>> = ids
                        .iter()
                        .map(|id| {
                            if id.is_empty() {
                                None
                            } else {
                                title(id).map(|t| format!("[{t}](card:{id})"))
                            }
                        })
                        .collect();
                    titles.map(|t| t.join(", ")).unwrap_or(cell)
                })
                .collect();
            out.push_str(&format!("| {} |", cells.join(" | ")));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// Markdown → простий текст для пошуку, уривків у результатах і ШІ:
/// без `#`, `**`, рамок таблиць і адрес посилань.
pub fn markdown_to_plain(md: &str) -> String {
    let md = strip_comments(md);
    let mut out = String::with_capacity(md.len());
    for line in md.lines() {
        let l = line.trim();
        if l.is_empty() || is_table_separator(l) {
            if l.is_empty() {
                out.push('\n');
            }
            continue;
        }
        let l = if l.starts_with('|') {
            table_cells(l)
                .into_iter()
                .filter(|c| !c.is_empty())
                .collect::<Vec<_>>()
                .join(" · ")
        } else {
            let l = l.trim_start_matches('#').trim_start();
            let l = l
                .strip_prefix("- ")
                .or_else(|| l.strip_prefix("* "))
                .or_else(|| l.strip_prefix("> "))
                .unwrap_or(l);
            l.to_string()
        };
        out.push_str(&inline_plain(&l));
        out.push('\n');
    }
    // Не більше одного порожнього рядка підряд.
    let mut clean = String::with_capacity(out.len());
    let mut blank = 0;
    for line in out.lines() {
        if line.trim().is_empty() {
            blank += 1;
            if blank > 1 || clean.is_empty() {
                continue;
            }
        } else {
            blank = 0;
        }
        clean.push_str(line);
        clean.push('\n');
    }
    clean.trim_end().to_string()
}

/// `**жирний**`, `` `код` `` і `[текст](адреса)` → просто текст.
fn inline_plain(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('[') {
        out.push_str(&rest[..p]);
        let after = &rest[p + 1..];
        match after.find("](") {
            Some(e) if !after[..e].contains('[') => {
                let tail = &after[e + 2..];
                match tail.find(')') {
                    Some(c) => {
                        out.push_str(&after[..e]);
                        rest = &tail[c + 1..];
                    }
                    None => {
                        out.push('[');
                        rest = after;
                    }
                }
            }
            _ => {
                out.push('[');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out.replace("**", "")
        .replace("__", "")
        .replace('`', "")
        .replace("\\", "")
}

/// HTML статті ZIM → Markdown для показу в застосунку: заголовки, абзаци,
/// списки, рядки таблиць (клітинки через « · »), жирний текст і посилання
/// на інші статті. `link` перетворює `href` на адресу в застосунку або
/// повертає `None` (тоді лишається лише текст посилання).
pub fn html_to_markdown(html: &str, link: &dyn Fn(&str) -> Option<String>) -> String {
    const SKIP: &[&str] = &[
        "head", "script", "style", "noscript", "sup", "template", "svg",
    ];
    let mut out = String::with_capacity(html.len() / 2);
    let mut links: Vec<Option<(usize, String)>> = Vec::new();
    let mut bold: Vec<usize> = Vec::new();
    let mut i = 0;
    let bytes = html.as_bytes();
    while i < bytes.len() {
        if bytes[i] != b'<' {
            let next = html[i..].find('<').map(|p| i + p).unwrap_or(bytes.len());
            let text = decode_entities(&html[i..next]);
            let mut prev_space = out.ends_with([' ', '\n']) || out.is_empty();
            for ch in text.chars() {
                if ch.is_whitespace() {
                    if !prev_space {
                        out.push(' ');
                        prev_space = true;
                    }
                    continue;
                }
                prev_space = false;
                if matches!(ch, '*' | '[' | ']' | '\\' | '#' | '|' | '`') {
                    out.push('\\');
                }
                out.push(ch);
            }
            i = next;
            continue;
        }
        if html[i..].starts_with("<!--") {
            i = html[i..]
                .find("-->")
                .map(|p| i + p + 3)
                .unwrap_or(bytes.len());
            continue;
        }
        let end = html[i..]
            .find('>')
            .map(|p| i + p + 1)
            .unwrap_or(bytes.len());
        let raw = &html[i..end];
        i = end;
        let closing = raw.starts_with("</");
        let name: String = raw
            .trim_start_matches('<')
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        if !closing && SKIP.contains(&name.as_str()) && !raw.ends_with("/>") {
            let close = format!("</{name}");
            let lower = html[i..].to_ascii_lowercase();
            i = match lower.find(&close) {
                Some(p) => html[i + p..]
                    .find('>')
                    .map(|q| i + p + q + 1)
                    .unwrap_or(bytes.len()),
                None => bytes.len(),
            };
            continue;
        }
        match name.as_str() {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                out.push_str("\n\n");
                if !closing {
                    let level = name[1..].parse::<usize>().unwrap_or(2).min(3);
                    out.push_str(&"#".repeat(level));
                    out.push(' ');
                }
            }
            "p" | "div" | "section" | "figure" | "blockquote" | "table" | "ul" | "ol" | "dl"
            | "figcaption" | "caption" | "header" | "footer" | "details" | "summary" => {
                out.push_str("\n\n")
            }
            "br" | "tr" | "dd" => out.push('\n'),
            "li" if !closing => out.push_str("\n- "),
            "dt" if !closing => out.push('\n'),
            "td" | "th" if !closing => {
                let line = out.rsplit('\n').next().unwrap_or("");
                if !line.trim().is_empty() {
                    out.push_str(" · ");
                }
            }
            "b" | "strong" => {
                if closing {
                    if let Some(pos) = bold.pop() {
                        if out[pos..].trim().is_empty() {
                            out.truncate(pos);
                        } else {
                            let trailing = out.len() - out.trim_end().len();
                            let tail: String = out[out.len() - trailing..].to_string();
                            out.truncate(out.len() - trailing);
                            out.push_str("**");
                            out.push_str(&tail);
                        }
                    }
                } else {
                    bold.push(out.len());
                    out.push_str("**");
                }
            }
            "a" => {
                if closing {
                    if let Some(Some((pos, target))) = links.pop() {
                        if out[pos + 1..].trim().is_empty() {
                            out.remove(pos);
                        } else {
                            out.push_str(&format!("]({target})"));
                        }
                    }
                } else {
                    let target = attr(raw, "href").and_then(|h| link(&decode_entities(&h)));
                    match target {
                        Some(t) => {
                            links.push(Some((out.len(), t)));
                            out.push('[');
                        }
                        None => links.push(None),
                    }
                }
            }
            _ => {}
        }
    }
    // Рядки без зайвих пробілів, не більше одного порожнього рядка підряд.
    let mut clean = String::with_capacity(out.len());
    let mut blank = true;
    for line in out.lines() {
        let l = line.trim();
        let l = l.strip_prefix("· ").unwrap_or(l).trim();
        if l.is_empty() || l == "-" || l == "**" || l == "·" {
            if !blank {
                clean.push('\n');
                blank = true;
            }
            continue;
        }
        blank = false;
        clean.push_str(l);
        clean.push('\n');
    }
    clean.trim_end().to_string()
}

/// Значення атрибута тега (`href="…"` чи `href='…'`).
fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(p) = lower[from..].find(name) {
        let start = from + p;
        from = start + name.len();
        let before_ok = start > 0 && lower.as_bytes()[start - 1].is_ascii_whitespace();
        let rest = tag[from..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        let rest = rest[1..].trim_start();
        let q = rest.chars().next()?;
        return if q == '"' || q == '\'' {
            rest[1..].find(q).map(|e| rest[1..1 + e].to_string())
        } else {
            Some(
                rest.chars()
                    .take_while(|c| !c.is_whitespace() && *c != '>')
                    .collect(),
            )
        };
    }
    None
}

/// Відносне посилання зі сторінки ZIM `base` (шлях без простору імен) →
/// шлях цільової сторінки. `None` для зовнішніх посилань, якорів і файлів.
pub fn resolve_zim_link(base: &str, href: &str) -> Option<String> {
    let h = href.trim();
    if h.is_empty()
        || h.starts_with('#')
        || h.starts_with("//")
        || h.contains("://")
        || h.starts_with("mailto:")
        || h.starts_with("javascript:")
    {
        return None;
    }
    let h = h.split(['#', '?']).next()?;
    let h = percent_decode(h);
    let lower = h.to_lowercase();
    if [
        ".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp", ".css", ".js", ".pdf",
    ]
    .iter()
    .any(|e| lower.ends_with(e))
    {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    if !h.starts_with('/') {
        parts.extend(base.split('/'));
        parts.pop(); // сама сторінка
    }
    for seg in h.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
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
    fn markdown_becomes_plain() {
        let md = "# Вода\n\n## Запас\n\n**Кип'ятіння** — 10 хв.\n<!-- потрібне першоджерело -->\n\n| № | Стан |\n|---|---|\n| 19 | [Опіки](card:fa-burns) |\n- пункт";
        let p = markdown_to_plain(md);
        assert_eq!(
            p,
            "Вода\n\nЗапас\n\nКип'ятіння — 10 хв.\n\n№ · Стан\n19 · Опіки\nпункт"
        );
        assert!(!p.contains("першоджерело"));
    }

    #[test]
    fn card_ids_become_links() {
        let md = "| 1 | Зупинка серця | fa-cpr, fa-aed |\n|---|---|---|\n| 2 | Інше | щось |";
        let t = |id: &str| match id {
            "fa-cpr" => Some("Реанімація".to_string()),
            "fa-aed" => Some("АЗД".to_string()),
            _ => None,
        };
        let out = link_card_ids(md, &t);
        assert!(
            out.contains("[Реанімація](card:fa-cpr), [АЗД](card:fa-aed)"),
            "{out}"
        );
        assert!(out.contains("| 2 | Інше | щось |"));
    }

    #[test]
    fn zim_html_becomes_markdown() {
        let html = "<html><head><title>Tourniquet</title></head><body><h1>Tourniquet</h1>\
            <table class=infobox><tr><th>Specialty</th><td><a href=\"Emergency_medicine\">Emergency medicine</a></td></tr></table>\
            <p>A <b>tourniquet</b> is a device<sup>[1]</sup>.</p><h2>Uses</h2><ul><li>One</li><li>Two</li></ul>\
            <img src=\"x.png\"><a href=\"https://example.org\">ext</a></body></html>";
        let md = html_to_markdown(html, &|h| {
            resolve_zim_link("A/Tourniquet", h).map(|p| format!("zim:p/{p}"))
        });
        assert_eq!(
            md,
            "# Tourniquet\n\nSpecialty · [Emergency medicine](zim:p/A/Emergency_medicine)\n\nA **tourniquet** is a device.\n\n## Uses\n\n- One\n- Two\n\next"
        );
        assert_eq!(
            resolve_zim_link("Tourniquet", "../A/Foo%20bar#x").as_deref(),
            Some("A/Foo bar")
        );
        assert_eq!(resolve_zim_link("Tourniquet", "Foo"), Some("Foo".into()));
        assert_eq!(resolve_zim_link("Tourniquet", "#cite"), None);
        let plain =
            html_to_text("<table><tr><th>Specialty</th><td>Emergency medicine</td></tr></table>");
        assert_eq!(plain, "Specialty · Emergency medicine\n");
    }

    #[test]
    fn chunks_respect_limit() {
        let text = "Перший абзац.\nДругий абзац.\nТретій.";
        let c = chunks(text, 15);
        assert_eq!(c.len(), 3);
        assert_eq!(chunks(text, 1000).len(), 1);
    }
}
