#!/usr/bin/env python3
"""Генерує іконки застосунку з brand/*.svg (Android, iOS, Windows, сайт).

Запуск з кореня репозиторію: python3 tools/brand_icons.py
Потрібні cairosvg і Pillow (pip install cairosvg pillow).
"""
import io
import json
import pathlib

import cairosvg
from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent.parent
BRAND = ROOT / "brand"
APP = ROOT / "app"


def png(svg: str, size: int, square: bool = False) -> Image.Image:
    data = (BRAND / svg).read_text()
    if square:
        # Для iOS: без заокруглення, система обрізає сама.
        data = data.replace('rx="28"', 'rx="0"')
    raw = cairosvg.svg2png(bytestring=data.encode(), output_width=size, output_height=size)
    return Image.open(io.BytesIO(raw)).convert("RGBA")


def mark(size: int, square: bool = False) -> Image.Image:
    return png("mark.svg" if size >= 64 else "mark-small.svg", size, square)


def main() -> None:
    res = APP / "android/app/src/main/res"
    for name, scale in {"mdpi": 1, "hdpi": 1.5, "xhdpi": 2, "xxhdpi": 3, "xxxhdpi": 4}.items():
        d = res / f"mipmap-{name}"
        d.mkdir(exist_ok=True)
        mark(int(48 * scale)).save(d / "ic_launcher.png")
        png("mark-foreground.svg", int(108 * scale)).save(d / "ic_launcher_foreground.png")
    anydpi = res / "mipmap-anydpi-v26"
    anydpi.mkdir(exist_ok=True)
    (anydpi / "ic_launcher.xml").write_text(
        '<?xml version="1.0" encoding="utf-8"?>\n'
        '<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">\n'
        '    <background android:drawable="@color/ic_launcher_background" />\n'
        '    <foreground android:drawable="@mipmap/ic_launcher_foreground" />\n'
        '    <monochrome android:drawable="@mipmap/ic_launcher_foreground" />\n'
        "</adaptive-icon>\n"
    )
    (res / "values/ic_launcher_background.xml").write_text(
        '<?xml version="1.0" encoding="utf-8"?>\n<resources>\n'
        '    <color name="ic_launcher_background">#1F4FD1</color>\n</resources>\n'
    )

    ios = APP / "ios/Runner/Assets.xcassets/AppIcon.appiconset"
    for img in json.loads((ios / "Contents.json").read_text())["images"]:
        f = img.get("filename")
        if not f:
            continue
        side = float(img["size"].split("x")[0]) * int(img["scale"].rstrip("x"))
        mark(round(side), square=True).convert("RGB").save(ios / f)

    big = mark(256)
    big.save(
        APP / "windows/runner/resources/app_icon.ico",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )

    site = ROOT / "site"
    (site / "favicon.svg").write_text((BRAND / "mark-small.svg").read_text())
    (site / "mark-on-blue.svg").write_text((BRAND / "mark-on-blue.svg").read_text())
    mark(180).save(site / "apple-touch-icon.png")
    mark(512).save(BRAND / "mark-512.png")


if __name__ == "__main__":
    main()
