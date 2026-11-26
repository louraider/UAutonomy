#!/usr/bin/env python3
"""Збирає SEO- і GEO-частини сайту uautonomy.app з одного джерела.

Що робить:
- вставляє розділ «Питання й відповіді» і JSON-LD у site/index.html
  (між позначками <!-- faq:start -->…<!-- faq:end --> та
  <!-- jsonld:start -->…<!-- jsonld:end -->);
- пише site/sitemap.xml;
- пише site/llms-full.txt: увесь видимий текст сайту одним файлом для ШІ-агентів.

Правте FAQ і PAGES тут, потім запустіть `python3 tools/site/build_seo.py`.
`--check` нічого не пише і падає, якщо файли на сайті відстали (так робить CI).
"""
import html
import json
import pathlib
import re
import sys
from html.parser import HTMLParser

ROOT = pathlib.Path(__file__).resolve().parents[2]
SITE = ROOT / "site"
BASE = "https://uautonomy.app"

# Сторінки, які мають бути в пошуку. Шлях → файл.
PAGES = [
    ("/", "index.html"),
    ("/privacy", "privacy.html"),
    ("/terms", "terms.html"),
]

# Кожна відповідь має бути зрозумілою без решти сторінки: її цитують пошук і ШІ.
# Лише перевірені факти; медичні застереження не послаблювати.
FAQ = [
    (
        "Чи «Автономія» справді безкоштовна?",
        "Так. Застосунок, карти, енциклопедії й ШІ-помічник безкоштовні, без реклами "
        "і без облікових записів. Застосунок не збирає і не надсилає даних про вас.",
    ),
    (
        "Чи працює застосунок без інтернету і світла?",
        "Так. Термінові картки доступні одразу після встановлення. Карти областей, "
        "енциклопедії й модель ШІ треба один раз завантажити, поки є інтернет, "
        "у вкладці «Пакети». Після цього все працює без мережі на заряді телефона чи ноутбука.",
    ),
    (
        "На яких пристроях працює «Автономія»?",
        "На телефонах Android (64-бітних, приблизно з 2017 року; для старих 32-бітних є "
        "окремий APK без ШІ-помічника) і на комп'ютерах з Windows 10 і 11. "
        "Версії для iPhone ще немає. Поки що iPhone може відкрити карти в браузері "
        "через «Обмін» з телефона чи ПК в тій самій мережі Wi-Fi.",
    ),
    (
        "Скільки місця займає застосунок?",
        "Сам застосунок невеликий. Найбільше місця займають пакети, які ви обираєте самі: "
        "карта своєї області, Вікіпедія, модель ШІ. Розмір кожного пакета видно перед "
        "завантаженням, тож можна взяти лише потрібне.",
    ),
    (
        "Як працює ШІ-помічник без інтернету?",
        "Мовна модель працює просто на пристрої: на телефоні Gemma 4 E2B, на комп'ютері "
        "Gemma 4 E4B або українська MamayLM 12B, залежно від потужності. Помічник "
        "відповідає українською за матеріалами бібліотеки і показує джерело. "
        "ШІ може помилятися; на медичні питання він показує картки.",
    ),
    (
        "Звідки медичні картки і чи їх перевірив лікар?",
        "Медичні картки складено за Наказом МОЗ №441 з дослівними цитатами. Жодну ще "
        "не перевірив лікар, а картки з мінної безпеки ще не перевірив сапер, і застосунок "
        "це прямо показує. «Автономія» не замінює лікаря, рятувальників чи офіційні "
        "сповіщення. У небезпеці телефонуйте 112.",
    ),
    (
        "Чи безпечно встановлювати APK з сайту?",
        "APK і інсталятор для Windows збирає відкритий CI на GitHub з відкритого коду "
        "(Apache-2.0). Контрольні суми SHA-256 опубліковано на uautonomy.app/sha256. "
        "Кожен пакет із вмістом підписано ключем проєкту, і застосунок не прийме "
        "підмінений файл.",
    ),
    (
        "Як передати застосунок і карти іншим без інтернету?",
        "Увімкніть «Обмін»: ваш телефон чи ПК роздає застосунок і пакети через локальний "
        "Wi-Fi, а інший пристрій заходить за QR-кодом. Так можна підготувати рідних чи "
        "сусідів, коли мобільний інтернет не працює.",
    ),
]

FEATURES = [
    "Картки «що робити зараз»: кровотеча, опіки, повітряна тривога, мінна небезпека, блекаут, вода, тепло",
    "Офлайн-карти всіх областей України, Києва і Криму на основі OpenStreetMap",
    "Українська Вікіпедія та медична енциклопедія без інтернету",
    "ШІ-помічник українською, що працює на пристрої",
    "Обмін застосунком і пакетами через локальний Wi-Fi і QR-код",
    "Підписані пакети вмісту",
]


def faq_html() -> str:
    items = []
    for q, a in FAQ:
        items.append(
            "        <details>\n"
            f"          <summary><h3>{html.escape(q, quote=False)}</h3></summary>\n"
            f"          <p>{html.escape(a, quote=False)}</p>\n"
            "        </details>"
        )
    return "\n".join(items)


def jsonld() -> str:
    org = {"@type": "Organization", "@id": f"{BASE}/#org", "name": "Автономія",
           "url": f"{BASE}/", "logo": f"{BASE}/apple-touch-icon.png",
           "sameAs": ["https://github.com/louraider/UAutonomy"]}
    data = {
        "@context": "https://schema.org",
        "@graph": [
            org,
            {"@type": "WebSite", "@id": f"{BASE}/#site", "url": f"{BASE}/",
             "name": "Автономія", "inLanguage": "uk", "publisher": {"@id": f"{BASE}/#org"}},
            {
                "@type": "SoftwareApplication",
                "@id": f"{BASE}/#app",
                "name": "Автономія",
                "alternateName": "UAutonomy",
                "description": "Безкоштовна офлайн-бібліотека для українців: картки «що робити зараз», "
                               "офлайн-карти України, енциклопедії та ШІ-помічник українською. "
                               "Працює без інтернету і світла.",
                "url": f"{BASE}/",
                "applicationCategory": "ReferenceApplication",
                "operatingSystem": "Android, Windows 10, Windows 11",
                "inLanguage": "uk",
                "isAccessibleForFree": True,
                "offers": {"@type": "Offer", "price": "0", "priceCurrency": "UAH"},
                "downloadUrl": [f"{BASE}/android", f"{BASE}/windows"],
                "license": "https://www.apache.org/licenses/LICENSE-2.0",
                "codeRepository": "https://github.com/louraider/UAutonomy",
                "featureList": FEATURES,
                "screenshot": f"{BASE}/og.png",
                "publisher": {"@id": f"{BASE}/#org"},
            },
            {
                "@type": "FAQPage",
                "@id": f"{BASE}/#faq",
                "mainEntity": [
                    {"@type": "Question", "name": q,
                     "acceptedAnswer": {"@type": "Answer", "text": a}}
                    for q, a in FAQ
                ],
            },
        ],
    }
    body = json.dumps(data, ensure_ascii=False, indent=1)
    # </script> усередині JSON зламав би сторінку.
    body = body.replace("</", "<\\/")
    return f'<script type="application/ld+json">\n{body}\n</script>'


def replace_between(text: str, name: str, content: str) -> str:
    start, end = f"<!-- {name}:start -->", f"<!-- {name}:end -->"
    pattern = re.compile(re.escape(start) + ".*?" + re.escape(end), re.S)
    if not pattern.search(text):
        sys.exit(f"index.html: немає позначок {start} / {end}")
    return pattern.sub(lambda m: f"{start}\n{content}\n{end}", text, count=1)


def sitemap() -> str:
    urls = "\n".join(
        f"  <url><loc>{BASE}{path}</loc></url>" for path, _ in PAGES
    )
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'
        f"{urls}\n</urlset>\n"
    )


class TextOf(HTMLParser):
    """Видимий текст <main> сторінки, з абзацами, заголовками і посиланнями Markdown."""

    BLOCKS = {"p", "li", "h1", "h2", "h3", "summary", "div", "section", "ol", "ul", "details"}
    SKIP = {"script", "style", "svg", "nav"}

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.out, self.buf, self.skip, self.inside, self.prefix = [], [], 0, 0, ""
        self._href = None

    def flush(self):
        line = re.sub(r"\s+", " ", "".join(self.buf)).strip()
        if line:
            self.out.append(self.prefix + line)
        self.buf, self.prefix = [], ""

    def handle_starttag(self, tag, attrs):
        if tag in self.SKIP:
            self.skip += 1
        if tag == "main":
            self.inside += 1
        if tag in self.BLOCKS:
            self.flush()
            self.prefix = {"h1": "# ", "h2": "## ", "h3": "### ", "li": "- "}.get(tag, "")
        if tag == "a" and self.inside and not self.skip:
            href = dict(attrs).get("href", "")
            if href.startswith("/") and not href.startswith("//"):
                href = BASE + href
            self.buf.append("[")
            self._href = href

    def handle_endtag(self, tag):
        if tag == "a" and self.inside and not self.skip and self._href is not None:
            self.buf.append(f"]({self._href})")
            self._href = None
        if tag in self.BLOCKS:
            self.flush()
        if tag in self.SKIP:
            self.skip -= 1
        if tag == "main":
            self.flush()
            self.inside -= 1

    def handle_data(self, data):
        if self.inside and not self.skip:
            self.buf.append(data)


def page_text(source: str) -> str:
    parser = TextOf()
    parser.feed(source)
    parser.flush()
    return "\n\n".join(parser.out)


def llms_full(index_html: str) -> str:
    title = re.search(r"<title>(.*?)</title>", index_html).group(1)
    desc = html.unescape(re.search(r'<meta name="description" content="(.*?)">', index_html).group(1))
    parts = [f"# {html.unescape(title)}", f"> {desc}",
             "Цей файл зібрано автоматично з тексту сторінок uautonomy.app "
             "(tools/site/build_seo.py). Короткий опис: " + BASE + "/llms.txt"]
    for path, name in PAGES:
        text = index_html if name == "index.html" else (SITE / name).read_text(encoding="utf-8")
        parts.append(f"---\n\nСторінка: {BASE}{path}\n\n{page_text(text)}")
    return "\n\n".join(parts) + "\n"


def main() -> None:
    check = "--check" in sys.argv[1:]
    index_path = SITE / "index.html"
    index = index_path.read_text(encoding="utf-8")
    new_index = replace_between(index, "faq", faq_html())
    new_index = replace_between(new_index, "jsonld", jsonld())
    outputs = {
        index_path: new_index,
        SITE / "sitemap.xml": sitemap(),
        SITE / "llms-full.txt": llms_full(new_index),
    }
    stale = []
    for path, content in outputs.items():
        current = path.read_text(encoding="utf-8") if path.exists() else None
        if current != content:
            stale.append(path.relative_to(ROOT))
            if not check:
                path.write_text(content, encoding="utf-8")
    if check and stale:
        sys.exit("Застарілі файли сайту: " + ", ".join(map(str, stale))
                 + ". Запустіть python3 tools/site/build_seo.py")
    print("оновлено: " + ", ".join(map(str, stale)) if stale else "усе актуальне")


if __name__ == "__main__":
    main()
