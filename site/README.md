# Сайт uautonomy.app

Статична сторінка проєкту: що таке «Автономія» і посилання на завантаження.
Без збирання, без зовнішніх шрифтів і трекерів. Розміщено на Vercel
(проєкт `uautonomy`, коренева тека `site/`), кожне злиття в `main` викладає
сайт автоматично.

Короткі адреси (`vercel.json`) ведуть на сталі файли релізів GitHub:

| Адреса | Куди |
|---|---|
| `/android`, `/apk` | `releases/download/app/avtonomiia.apk` |
| `/windows` | `releases/download/app/avtonomiia-windows-x64.zip` |
| `/sha256` | `releases/download/app/SHA256SUMS.txt` |
| `/catalog.json` | `releases/download/packs/catalog.json` (каталог пакетів, підписаний) |

Реліз `app` оновлює job `release` у `.github/workflows/ci.yml` після кожного
зеленого злиття в `main`. Сторінка через GitHub API показує дату й розмір
поточної збірки, а якщо релізу ще немає, так і пише.

Медичні тексти на сайті мають лишатися з позначкою «ще не перевірено
лікарем», доки картки не пройдуть рецензію (див. `docs/content-verification/`).
