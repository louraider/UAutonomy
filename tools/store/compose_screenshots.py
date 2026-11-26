#!/usr/bin/env python3
"""Складає знімки для Google Play і Microsoft Store у стилі «Мапа».

Сирі знімки робить app/test/store_screenshots_test.dart (див. коментар
там). Цей скрипт кладе кожен на синє тло з підписом і малює банер
1024 × 500 для Google Play:

    python3 tools/store/compose_screenshots.py <тека зі знімками> [вихідна тека]

Підписи і їхній порядок задано в SHOTS нижче.
"""
import pathlib
import sys

from PIL import Image, ImageDraw, ImageFont

ROOT = pathlib.Path(__file__).resolve().parents[2]
FONTS = ROOT / "app/assets/fonts"
BLUE = (31, 79, 209)
INK = (13, 27, 42)
YELLOW = (255, 208, 40)
WHITE = (255, 255, 255)

# (сирий знімок, заголовок, підзаголовок)
PHONE = [
    ("phone-1-library", "Що робити, коли немає зв'язку", "Домедична допомога, укриття, мінна безпека, блекаут"),
    ("phone-2-search", "Пошук без інтернету", "Уся довідка лежить на телефоні"),
    ("phone-3-card", "Покроково, за наказом МОЗ № 441", "Коротко, щоб прочитати під стресом"),
    ("phone-4-category", "Тривога, укриття, евакуація", "Що робити вдома, на вулиці й у дорозі"),
    ("phone-5-card-mine", "Знайшли підозрілий предмет?", "Картки з мінної безпеки"),
    ("phone-6-library-dark", "Темна тема", "Не світить у темряві й укритті"),
]
DESKTOP = [
    ("desktop-1-library", "Та сама довідка на комп'ютері", "Windows 10 і 11, без інтернету"),
    ("desktop-2-card", "Картки на весь екран", "Зручно читати вголос іншим"),
    ("desktop-3-share", "Обмін без інтернету", "Телефон сканує QR-код і отримує бібліотеку"),
]


def font(name: str, size: int) -> ImageFont.FreeTypeFont:
    return ImageFont.truetype(str(FONTS / name), size)


def rounded(img: Image.Image, radius: int) -> Image.Image:
    mask = Image.new("L", img.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, *img.size), radius, fill=255)
    out = Image.new("RGBA", img.size)
    out.paste(img, (0, 0), mask)
    return out


def wrap(draw, text, fnt, width):
    lines, line = [], ""
    for word in text.split():
        test = f"{line} {word}".strip()
        if draw.textlength(test, font=fnt) <= width:
            line = test
        else:
            lines.append(line)
            line = word
    lines.append(line)
    return lines


def compose(raw: Image.Image, title: str, sub: str, size, shot_w, top, title_px, sub_px, radius):
    W, H = size
    canvas = Image.new("RGB", size, BLUE)
    d = ImageDraw.Draw(canvas)
    ft = font("WixMadeforDisplay-800.ttf", title_px)
    fs = font("WixMadeforText-Medium.ttf", sub_px)
    y = int(top * 0.32)
    # Жовта мітка над заголовком, як маркер на мапі.
    d.rounded_rectangle((W // 2 - 40, y, W // 2 + 40, y + 12), 6, fill=YELLOW)
    y += 40
    for line in wrap(d, title, ft, W - 120):
        d.text((W // 2, y), line, font=ft, fill=WHITE, anchor="ma")
        y += int(title_px * 1.15)
    y += 10
    for line in wrap(d, sub, fs, W - 160):
        d.text((W // 2, y), line, font=fs, fill=(220, 229, 252), anchor="ma")
        y += int(sub_px * 1.3)
    shot = raw.convert("RGB")
    h = int(shot.height * shot_w / shot.width)
    shot = rounded(shot.resize((shot_w, h), Image.LANCZOS), radius)
    top = max(top, y + 40)
    # Тінь під знімком.
    shadow = Image.new("RGBA", (shot_w + 40, h + 40), (0, 0, 0, 0))
    ImageDraw.Draw(shadow).rounded_rectangle((20, 28, shot_w + 20, h + 20), radius, fill=(8, 24, 80, 90))
    canvas.paste(shadow, ((W - shot_w) // 2 - 20, top - 20), shadow)
    canvas.paste(shot, ((W - shot_w) // 2, top), shot)
    return canvas


def feature_graphic(out: pathlib.Path):
    W, H = 1024, 500
    img = Image.new("RGB", (W, H), BLUE)
    d = ImageDraw.Draw(img)
    mark = Image.open(ROOT / "brand/mark-512.png").convert("RGBA").resize((220, 220), Image.LANCZOS)
    img.paste(mark, (90, (H - 220) // 2), mark)
    d.text((360, 150), "Автономія", font=font("WixMadeforDisplay-800.ttf", 84), fill=WHITE)
    fs = font("WixMadeforText-Medium.ttf", 34)
    d.text((364, 262), "Довідка на випадок,", font=fs, fill=(220, 229, 252))
    d.text((364, 306), "коли немає зв'язку", font=fs, fill=(220, 229, 252))
    d.rounded_rectangle((364, 120, 444, 132), 6, fill=YELLOW)
    img.save(out / "play-feature-graphic-1024x500.png")


def main():
    src = pathlib.Path(sys.argv[1])
    out = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else src / "store"
    out.mkdir(parents=True, exist_ok=True)
    for i, (name, title, sub) in enumerate(PHONE, 1):
        raw = Image.open(src / f"{name}.png")
        compose(raw, title, sub, (1080, 1920), 820, 400, 68, 36, 44).save(out / f"phone-{i}.png")
    for i, (name, title, sub) in enumerate(DESKTOP, 1):
        raw = Image.open(src / f"{name}.png")
        compose(raw, title, sub, (1920, 1080), 1380, 230, 60, 30, 24).save(out / f"desktop-{i}.png")
    feature_graphic(out)
    Image.open(ROOT / "brand/mark-512.png").convert("RGB").save(out / "icon-512.png")
    print(f"готово: {out}")


if __name__ == "__main__":
    main()
