# iOS

Той самий Flutter-застосунок і те саме Rust-ядро, що на Android і Windows.
iOS-версія розповсюджується лише через App Store (TestFlight для тестів):
встановити застосунок на iPhone «з файлу», як APK, не можна.

## Що зроблено

| Частина | Як на iOS |
|---|---|
| Rust-ядро (пакети, пошук, підписи каталогу й пакетів Ed25519, карти PMTiles, обмін) | Статична бібліотека через cargokit (`app/rust_builder/ios`), цілі `aarch64-apple-ios` і `aarch64-apple-ios-sim` |
| ШІ-помічник (llama.cpp, GGUF) | `llamadart` + `llamadart_llama_cpp_flutter`: готовий XCFramework через Swift Package Manager, Metal на GPU. Потрібна iOS 16.4+ |
| Пам'ять для моделі | Entitlement `com.apple.developer.kernel.increased-memory-limit` (`Runner.entitlements`): модель ~3 ГБ інакше впирається в ліміт пам'яті застосунку |
| Пакети й моделі | У `Application Support`, виключені з копії iCloud (`excludeFromBackup` в `AppDelegate.swift`). Інакше гігабайти карт ішли б у iCloud користувача, а App Store відхиляє таке за правилами зберігання даних |
| Експорт пакета | У «Документи» застосунку: «Файли» → «На iPhone» → «Автономія» і Finder на Mac (`UIFileSharingEnabled`) |
| Імпорт пакета чи моделі | З «Файлів» (флешка через USB-C, iCloud Drive тощо). Тимчасову копію, яку робить iOS, прибираємо одразу після імпорту. Імпорт цілої теки на iOS вимкнено |
| Карти | Векторні PMTiles, як на Android |
| Обмін | Той самий сервер у локальній мережі й QR-код. iOS зупиняє сервер, коли застосунок іде у фон, тож поки обмін увімкнено, екран не гасне. APK для Android iPhone не роздає |
| Приватність | `PrivacyInfo.xcprivacy`: анонімна статистика за згодою (Product Interaction, не прив'язана до людини, без стеження), причини для API вільного місця (E174.1) і часу файлів (C617.1) |
| Шифрування | `ITSAppUsesNonExemptEncryption = false`: лише HTTPS і перевірка підписів, це звільнено від експортної документації |

CI (`.github/workflows/ci.yml`, job `ios`, macOS): `flutter build ios --release
--no-codesign`, артефакт `uautonomy-ios-unsigned` (IPA без підпису, на
iPhone його не встановити, лише для перевірки вмісту), і наскрізний тест
`integration_test/app_test.dart` на симуляторі iPhone разом із запуском
llama.cpp на крихітній моделі stories260K.

## Що потрібно від власника, щоб викласти в TestFlight і App Store

1. **Apple Developer Program** (99 $ на рік) на фізичну особу чи організацію.
   Для організації потрібен D-U-N-S. Ім'я в App Store буде тим, на кого
   оформлено акаунт.
2. **Bundle ID** `org.uautonomy.app` (такий самий, як applicationId на Android).
   Зареєструвати в Certificates, Identifiers & Profiles і ввімкнути
   capability **Increased Memory Limit**.
3. **Запис застосунку в App Store Connect** з назвою «Автономія», мовою
   українською, категорією (Довідники чи Освіта), політикою приватності
   (https://uautonomy.app/terms) і вікова оцінка.
4. **Ключ App Store Connect API** (роль App Manager): `.p8`, Key ID, Issuer ID.
   З ним CI сам підписує (автоматичне керування сертифікатами) і вантажить
   збірку в TestFlight, без Mac у власника.
   Секрети репозиторію: `APPSTORE_API_KEY_P8_BASE64`, `APPSTORE_API_KEY_ID`,
   `APPSTORE_API_ISSUER_ID`, а також `APPLE_TEAM_ID`.
5. Пройти анкету експортного контролю (відповідь: лише звільнене шифрування)
   і заповнити «App Privacy» в App Store Connect (Usage Data → Product Interaction, Analytics, Not linked to user, No tracking; Location → Coarse Location, бо Aptabase визначає країну за IP). Збірку з ключем статистики треба запускати з `--dart-define=UAUTONOMY_STATS_KEY=…`, як на Android і Windows.

Після цього додається крок CI `xcodebuild -exportArchive` з підписом
і `xcrun altool --upload-app` (або `fastlane pilot`) у TestFlight.

## Відомі обмеження

- Завантаження пакетів зупиняється, коли застосунок іде у фон. Докачування
  продовжується з того самого місця, коли застосунок знову відкрито.
- Модель Gemma 4 E2B (~3 ГБ) розрахована на iPhone з 6 ГБ пам'яті й більше
  (iPhone 12 Pro / 13 Pro / 14 / 15 і новіші). На старіших працюватимуть
  довідка, карти й картки, але не помічник.
- Обмін працює, лише поки застосунок відкритий на екрані.
