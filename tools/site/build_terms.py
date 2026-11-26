#!/usr/bin/env python3
"""Збирає site/terms.html з app/assets/legal/terms_uk.md.

Один текст умов для застосунку і сайту: правте Markdown, потім запустіть
`python3 tools/site/build_terms.py`. Редакцію беремо з app/lib/legal/terms.dart.
"""
import html
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
MD = ROOT / "app/assets/legal/terms_uk.md"
DART = ROOT / "app/lib/legal/terms.dart"
OUT = ROOT / "site/terms.html"

LINKS = {
    "ai.google.dev/gemma/terms": "https://ai.google.dev/gemma/terms",
    "ai.google.dev/gemma/prohibited_use_policy": "https://ai.google.dev/gemma/prohibited_use_policy",
    "github.com/louraider/UAutonomy/issues": "https://github.com/louraider/UAutonomy/issues",
    "uautonomy.app/terms": "https://uautonomy.app/terms",
}


def inline(s: str) -> str:
    s = html.escape(s, quote=False)
    s = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", s)
    for text, url in LINKS.items():
        s = re.sub(rf"(?<![\w/.]){re.escape(text)}(?![\w/])", f'<a href="{url}">{text}</a>', s)
    return s


def body(md: str) -> str:
    out, para, items = [], [], []

    def flush():
        if para:
            out.append(f"<p>{inline(' '.join(para))}</p>")
            para.clear()
        if items:
            out.append("<ul>\n" + "\n".join(f"  <li>{inline(i)}</li>" for i in items) + "\n</ul>")
            items.clear()

    for raw in md.splitlines():
        line = raw.strip()
        if not line:
            flush()
            continue
        m = re.match(r"^(#{1,3})\s+(.*)$", line)
        if m:
            flush()
            lvl = len(m.group(1))
            out.append(f"<h{lvl}>{inline(m.group(2))}</h{lvl}>")
        elif line.startswith("- "):
            if para:
                flush()
            items.append(line[2:])
        else:
            if items:
                flush()
            para.append(line)
    flush()
    return "\n".join(out)


def main():
    version = re.search(r"termsVersion = '([^']+)'", DART.read_text()).group(1)
    content = body(MD.read_text())
    OUT.write_text(TEMPLATE.replace("{{VERSION}}", version).replace("{{BODY}}", content))
    print(f"{OUT.relative_to(ROOT)}: редакція {version}")


TEMPLATE = """<!doctype html>
<!-- Згенеровано tools/site/build_terms.py з app/assets/legal/terms_uk.md. Не правте вручну. -->
<html lang="uk">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>Умови використання · Автономія</title>
<meta name="description" content="Умови використання застосунку «Автономія»: медичне застереження, обмеження використання моделей ШІ (Gemma), ліцензії і дані.">
<link rel="canonical" href="https://uautonomy.app/terms">
<meta name="theme-color" content="#1F4FD1">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
<link rel="stylesheet" href="/style.css">
<script src="/ga.js"></script>
</head>
<body>
<a class="skip" href="#main">До змісту</a>
<header class="doc-top">
  <div class="wrap top">
    <a class="brand" href="/"><img src="/mark-on-blue.svg" alt=""><span>автономія</span></a>
  </div>
</header>
<main id="main" class="wrap doc" data-terms-version="{{VERSION}}">
{{BODY}}
</main>
<footer>
  <div class="wrap">
    <a class="brand" href="/"><img src="/favicon.svg" alt="">автономія</a>
    <span>Повні тексти англійською: <a href="https://ai.google.dev/gemma/terms">Gemma Terms of Use</a>, <a href="https://ai.google.dev/gemma/prohibited_use_policy">Gemma Prohibited Use Policy</a></span>
  </div>
</footer>
</body>
</html>
"""

if __name__ == "__main__":
    main()
