# Сайт uautonomy.app

Статична сторінка проєкту: що таке «Автономія» і посилання на завантаження.
Без зовнішніх шрифтів. Єдиний сторонній скрипт: Google Analytics 4
(`ga.js`, ресурс `G-8RQB5YYHY7`), без рекламних сигналів; не вантажиться при
GPC/Do Not Track чи після вимкнення на `/privacy`. Застосунок має власну анонімну статистику лише за згодою людини
(`app/lib/stats/stats.dart`, Aptabase); що вона надсилає, описано на `/privacy`.
`robots.txt` і `sitemap.xml` відкривають сайт для пошуковиків і ШІ-асистентів;
додаючи сторінку, допишіть її в `PAGES` у `tools/site/build_seo.py`
(див. «Пошук і ШІ-асистенти» нижче). Розміщено на Vercel
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

## Пошук і ШІ-асистенти

- `robots.txt` відкритий для всіх ботів, `sitemap.xml` перелічує сторінки.
- `llms.txt` — короткий опис проєкту для ШІ-асистентів, пишемо вручну.
  Якщо змінюються адреси завантаження чи застереження, правте і його.
- Розділ «Питання й відповіді», JSON-LD (`SoftwareApplication`, `FAQPage`),
  `sitemap.xml` і `llms-full.txt` збирає `python3 tools/site/build_seo.py`.
  Питання правте в самому скрипті, а після будь-якої зміни тексту сторінок
  запускайте його знову. CI (`build_seo.py --check`) падає, якщо файли відстали.
- Нову сторінку додавайте в `PAGES` у тому ж скрипті, щоб вона потрапила в
  sitemap і `llms-full.txt`, і дайте їй `<link rel="canonical">`.
