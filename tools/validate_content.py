#!/usr/bin/env python3
"""Перевірка формату content/ і eval/questions.jsonl.

Запуск з кореня репозиторію:  python tools/validate_content.py
Лише стандартна бібліотека Python 3.9+. Код виходу 1, якщо є помилки.
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CONTENT = ROOT / "content"
EVAL = ROOT / "eval" / "questions.jsonl"

CATEGORIES = {"first_aid", "civil_protection", "mine_safety", "household", "communication"}
REVIEW = {None, "medic", "deminer"}
PREFIX = {"first_aid": "fa-", "civil_protection": "cp-", "mine_safety": "ms-",
          "household": "hh-", "communication": "cm-"}
CARD_REQUIRED = ["id", "category", "title", "summary", "steps", "source", "source_ids",
                 "source_ref", "review_required", "reviewed", "updated"]
SOURCE_REQUIRED = ["id", "title", "publisher", "kind", "urls", "accessed", "legal_status", "license"]
LEGAL = {"not_protected_official_document", "facts_restated", "permission_required", "licensed"}
EVAL_TYPES = {"retrieval", "generation", "safety"}
BEHAVIOURS = {"answer", "refuse", "redirect", "no_answer"}
DATE = re.compile(r"^\d{4}-\d{2}-\d{2}$")

errors: list[str] = []


def err(where: str, msg: str) -> None:
    errors.append(f"{where}: {msg}")


def parse_frontmatter(text: str) -> dict:
    """Мінімальний YAML: key: value, списки [a, b], null/true/false."""
    if not text.startswith("---\n"):
        raise ValueError("немає YAML-фронтматера")
    end = text.find("\n---", 4)
    if end < 0:
        raise ValueError("фронтматер не закрито")
    meta: dict = {}
    for line in text[4:end].splitlines():
        if not line.strip():
            continue
        key, _, val = line.partition(":")
        val = val.strip()
        if val.startswith("[") and val.endswith("]"):
            meta[key.strip()] = [v.strip() for v in val[1:-1].split(",") if v.strip()]
        elif val in ("null", "~", ""):
            meta[key.strip()] = None
        elif val in ("true", "false"):
            meta[key.strip()] = val == "true"
        else:
            meta[key.strip()] = val
    return meta


def main() -> int:
    sources = json.loads((CONTENT / "sources.json").read_text(encoding="utf-8"))
    source_ids = set()
    for s in sources:
        for k in SOURCE_REQUIRED:
            if k not in s:
                err(f"sources.json[{s.get('id')}]", f"немає поля {k}")
        if s.get("legal_status") not in LEGAL:
            err(f"sources.json[{s.get('id')}]", f"невідомий legal_status {s.get('legal_status')}")
        if s["id"] in source_ids:
            err("sources.json", f"дубль id {s['id']}")
        source_ids.add(s["id"])

    all_ids: dict[str, str] = {}

    def register(id_: str, where: str) -> None:
        if id_ in all_ids:
            err(where, f"id {id_} уже є в {all_ids[id_]}")
        all_ids[id_] = where

    def check_common(meta: dict, where: str, needs_review_for_category: bool = True) -> None:
        cat = meta.get("category")
        if cat not in CATEGORIES:
            err(where, f"невідома категорія {cat}")
        if meta.get("review_required") not in REVIEW:
            err(where, f"review_required має бути medic, deminer або null")
        if needs_review_for_category:
            if cat == "first_aid" and meta.get("review_required") != "medic":
                err(where, "first_aid потребує review_required: medic")
            if cat == "mine_safety" and meta.get("review_required") != "deminer":
                err(where, "mine_safety потребує review_required: deminer")
        if meta.get("reviewed") is True and meta.get("review_required") and not meta.get("reviewed_by"):
            err(where, "reviewed=true без reviewed_by")
        for sid in meta.get("source_ids") or []:
            if sid not in source_ids:
                err(where, f"невідоме джерело {sid}")
        if not meta.get("source_ids"):
            err(where, "порожній source_ids")
        if not DATE.match(str(meta.get("updated", ""))):
            err(where, "updated має бути YYYY-MM-DD")

    n_cards = 0
    for path in sorted((CONTENT / "cards").glob("*.json")):
        cards = json.loads(path.read_text(encoding="utf-8"))
        for c in cards:
            where = f"{path.relative_to(ROOT)}[{c.get('id')}]"
            n_cards += 1
            for k in CARD_REQUIRED:
                if k not in c:
                    err(where, f"немає поля {k}")
            if not isinstance(c.get("steps"), list) or not c.get("steps"):
                err(where, "steps має бути непорожнім списком")
            if c.get("category") != path.stem:
                err(where, f"категорія {c.get('category')} не збігається з файлом {path.name}")
            pre = PREFIX.get(c.get("category", ""), "")
            if pre and not str(c.get("id", "")).startswith(pre):
                err(where, f"id має починатися з {pre}")
            check_common(c, where)
            register(c["id"], where)

    n_articles = 0
    for path in sorted((CONTENT / "articles").rglob("*.md")):
        where = str(path.relative_to(ROOT))
        try:
            meta = parse_frontmatter(path.read_text(encoding="utf-8"))
        except ValueError as e:
            err(where, str(e))
            continue
        n_articles += 1
        for k in ("id", "title", "category", "source_ids", "review_required", "reviewed", "updated"):
            if k not in meta:
                err(where, f"немає поля {k}")
        if meta.get("category") != path.parent.name:
            err(where, "категорія не збігається з текою")
        check_common(meta, where)
        register(meta["id"], where)

    n_q = 0
    counts = {t: 0 for t in EVAL_TYPES}
    if EVAL.exists():
        seen = set()
        for i, line in enumerate(EVAL.read_text(encoding="utf-8").splitlines(), 1):
            if not line.strip():
                continue
            where = f"eval/questions.jsonl:{i}"
            try:
                q = json.loads(line)
            except json.JSONDecodeError as e:
                err(where, f"невірний JSON: {e}")
                continue
            n_q += 1
            if q.get("id") in seen:
                err(where, f"дубль id {q.get('id')}")
            seen.add(q.get("id"))
            if q.get("type") not in EVAL_TYPES:
                err(where, f"невідомий type {q.get('type')}")
            else:
                counts[q["type"]] += 1
            if q.get("expected_behavior") not in BEHAVIOURS:
                err(where, f"невідомий expected_behavior {q.get('expected_behavior')}")
            if not q.get("question"):
                err(where, "порожнє питання")
            for cid in q.get("expected_ids", []):
                if cid not in all_ids:
                    err(where, f"expected_ids посилається на невідомий id {cid}")
            if q.get("type") == "generation" and not q.get("reference_answer"):
                err(where, "generation без reference_answer")
            for grp in q.get("must_include", []):
                if not isinstance(grp, list) or not grp:
                    err(where, "must_include: кожен елемент має бути непорожнім списком варіантів")

    for e in errors:
        print("ПОМИЛКА", e)
    print(f"Карток: {n_cards}, статей: {n_articles}, джерел: {len(sources)}, питань: {n_q} {counts}")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
