# Застосунок UAutonomy (Flutter)

Інтерфейс для Android, Windows та iOS (див. ../docs/ios.md). Логіка в Rust-ядрі `../core`, підключеному через `flutter_rust_bridge` (crate-міст у `rust/`, збірка через cargokit у `rust_builder/`).

```bash
flutter pub get
flutter run                       # на підключеному пристрої
flutter build apk --release       # Android
flutter build windows --release   # Windows (на Windows)
```

Після зміни `rust/src/api/` перегенеруйте міст:

```bash
cargo install flutter_rust_bridge_codegen --version 2.13.0 --locked
flutter_rust_bridge_codegen generate
```
