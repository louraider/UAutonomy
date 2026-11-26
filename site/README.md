# Сайт uautonomy.app

Статична сторінка проєкту: що таке «Автономія» і посилання на завантаження.
Без зовнішніх шрифтів. Єдиний сторонній скрипт: Google Analytics 4
(`ga.js`, ресурс `G-8RQB5YYHY7`), без рекламних сигналів; не вантажиться при
GPC/Do Not Track чи після вимкнення на `/privacy`. Застосунок аналітики не має.
`robots.txt` і `sitemap.xml` відкривають сайт для пошуковиків і ШІ-асистентів;
додаючи сторінку, допишіть її в `sitemap.xml`. Розміщено на Vercel
(проєкт `uautonomy`, коренева тека `site/`), кожне злиття в `main` викладає
сайт автоматично.

Короткі адреси (`vercel.json`) ведуть на сталі файли релізів GitHub:

| Адреса | Куди |
|---|---|
| `/android`, `/apk` | `releases/download/app/avtonomiia.apk` |
| `/android-32` | `releases/download/app/avtonomiia-armeabi-v7a.apk` |
| `/android-x86` | `releases/download/app/avtonomiia-x86_64.apk` |
| `/windows` | `releases/download/app/avtonomiia-windows-x64.zip` |
| `/sha256` | `releases/download/app/SHA256SUMS.txt` |
| `/catalog.json` | `releases/download/packs/catalog.json` (каталог пакетів, підписаний) |

Реліз `app` оновлює job `release` у `.github/workflows/ci.yml` після кожного
зеленого злиття в `main`. Сторінка через GitHub API показує дату й розмір
поточної збірки, а якщо релізу ще немає, так і пише.

Медичні тексти на сайті мають лишатися з позначкою «ще не перевірено
лікарем», доки картки не пройдуть рецензію (див. `docs/content-verification/`).

Умови використання (`/terms`) збираються з того самого тексту, що й у
застосунку: правте `app/assets/legal/terms_uk.md` і запускайте
`python3 tools/site/build_terms.py`. Тест `app/test/terms_test.dart` падає,
якщо сайт відстав від застосунку.
