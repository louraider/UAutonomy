#!/usr/bin/env python3
"""Збирає вшитий базовий пакет із теки content/.

Читає content/cards/*.json (масиви карток) і content/articles/**/*.md
та пише core/crates/uautonomy-core/assets/base/{cards,articles}.json.
Ядро вбудовує ці файли в застосунок і хаб (include_str!).

    python3 tools/build_base_pack.py [--content content] [--check]

--check: лише перевірити (для CI), нічого не писати.
"""
import argparse
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "core/crates/uautonomy-core/assets/base"
CATEGORIES = {"first_aid", "civil_protection", "mine_safety", "household", "communication"}
REQUIRED = ("id", "category", "title", "steps", "source")


def main() -> int:
    # Консоль Windows у CI — cp1252; друкуємо українською в UTF-8.
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser()
    ap.add_argument("--content", default=str(ROOT / "content"))
    ap.add_argument("--check", action="store_true")
    a = ap.parse_args()
    content = pathlib.Path(a.content)
    errors = []
    cards, ids = [], set()
    for f in sorted((content / "cards").glob("*.json")):
        for c in json.loads(f.read_text(encoding="utf-8")):
            missing = [k for k in REQUIRED if k not in c]
            if missing:
                errors.append(f"{f.name}: {c.get('id')}: бракує {missing}")
                continue
            if c["category"] not in CATEGORIES:
                errors.append(f"{f.name}: {c['id']}: невідома категорія {c['category']}")
            if c["id"] in ids:
                errors.append(f"{f.name}: повторний id {c['id']}")
            ids.add(c["id"])
            cards.append(c)
    articles = []
    for f in sorted((content / "articles").glob("**/*.md")):
        aid = f.stem
        if aid in ids:
            errors.append(f"{f}: id {aid} уже зайнятий карткою")
        articles.append([aid, f.read_text(encoding="utf-8")])
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    if not cards:
        print("content/cards порожня: лишаю демонстраційні картки", file=sys.stderr)
        return 0
    print(f"{len(cards)} карток, {len(articles)} статей")
    if not a.check:
        OUT.mkdir(parents=True, exist_ok=True)
        (OUT / "cards.json").write_text(json.dumps(cards, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        (OUT / "articles.json").write_text(json.dumps(articles, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
