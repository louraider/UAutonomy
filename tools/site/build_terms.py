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
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Умови використання · Автономія</title>
<meta name="description" content="Умови використання застосунку «Автономія»: медичне застереження, обмеження використання моделей ШІ (Gemma), ліцензії і дані.">
<meta name="theme-color" content="#1f4f8f">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<style>
  :root { --bg: #f6f4ef; --surface: #ffffff; --text: #1b1d22; --muted: #5a606b; --line: #e2ded4; --accent: #1f4f8f; }
  @media (prefers-color-scheme: dark) {
    :root { --bg: #121417; --surface: #1b1e23; --text: #eceef1; --muted: #a3a9b3; --line: #2c3038; --accent: #6ea3e8; }
  }
  * { box-sizing: border-box; }
  html { -webkit-text-size-adjust: 100%; }
  body { margin: 0; background: var(--bg); color: var(--text);
    font: 17px/1.6 system-ui, -apple-system, "Segoe UI", Roboto, "Noto Sans", Ubuntu, sans-serif; }
  a { color: var(--accent); overflow-wrap: anywhere; }
  .wrap { max-width: 760px; margin: 0 auto; padding: 0 16px; }
  header.top { display: flex; align-items: center; gap: 10px; padding: 18px 0; }
  header.top img { width: 34px; height: 34px; }
  header.top a.name { font-weight: 700; font-size: 19px; color: var(--text); text-decoration: none; }
  h1 { font-size: clamp(28px, 5vw, 40px); line-height: 1.15; margin: 18px 0 8px; }
  h2 { font-size: 22px; margin: 34px 0 8px; padding-top: 18px; border-top: 1px solid var(--line); }
  ul { padding-left: 22px; }
  li { margin: 6px 0; }
  footer { padding: 30px 0 46px; color: var(--muted); font-size: 14.5px; border-top: 1px solid var(--line); margin-top: 34px; }
</style>
</head>
<body>
<div class="wrap">
  <header class="top">
    <img src="/favicon.svg" alt="">
    <a class="name" href="/">Автономія</a>
  </header>
  <main data-terms-version="{{VERSION}}">
{{BODY}}
  </main>
  <footer>
    <a href="/">На головну</a> · Повні тексти англійською: <a href="https://ai.google.dev/gemma/terms">Gemma Terms of Use</a>, <a href="https://ai.google.dev/gemma/prohibited_use_policy">Gemma Prohibited Use Policy</a>
  </footer>
</div>
</body>
</html>
"""

if __name__ == "__main__":
    main()
