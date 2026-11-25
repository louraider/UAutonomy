// Знімки екранів для Google Play і Microsoft Store зі справжньою
// бібліотекою (Rust-ядро, картки з content/), а не з прикладу.
//
// Запуск (Linux чи macOS, з кореня репозиторію):
//   python3 tools/build_base_pack.py
//   (cd app/rust && cargo build --release)
//   cd app && STORE_SHOTS=/тека UAUTONOMY_RUST_LIB=$PWD/rust/target/release/librust_lib_uautonomy.so \
//     flutter test test/store_screenshots_test.dart
//   python3 tools/store/compose_screenshots.py /тека
//
// Без STORE_SHOTS і UAUTONOMY_RUST_LIB тест пропускається.
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/main.dart';
import 'package:uautonomy/screens/library_screen.dart';
import 'package:uautonomy/src/rust/api/library.dart';
import 'package:uautonomy/src/rust/api/share.dart';
import 'package:uautonomy/src/rust/frb_generated.dart';
import 'package:uautonomy/theme.dart';

final _outDir = Platform.environment['STORE_SHOTS'];
final _rustLib = Platform.environment['UAUTONOMY_RUST_LIB'];

/// Логічний розмір телефона; знімок у 3 рази більший (1170 × 2280).
const _phone = Size(390, 760);

/// Вікно ПК для Microsoft Store (знімок 1920 × 1080 при масштабі 1.875).
const _desktop = Size(1024, 576);

Future<void> _loadFonts() async {
  Future<void> family(String name, List<String> files) async {
    final loader = FontLoader(name);
    for (final f in files) {
      loader.addFont(
        File(f).readAsBytes().then((b) => ByteData.sublistView(b)),
      );
    }
    await loader.load();
  }

  await family(displayFont, [
    'assets/fonts/WixMadeforDisplay-700.ttf',
    'assets/fonts/WixMadeforDisplay-800.ttf',
  ]);
  await family(textFont, [
    for (final w in ['Regular', 'Medium', 'SemiBold', 'Bold'])
      'assets/fonts/WixMadeforText-$w.ttf',
  ]);
  await family(monoFont, ['assets/fonts/IBMPlexMono-Medium.ttf']);
  final root = Platform.environment['FLUTTER_ROOT'];
  final icons = File(
    '$root/bin/cache/artifacts/material_fonts/MaterialIcons-Regular.otf',
  );
  if (root != null && icons.existsSync()) {
    await family('MaterialIcons', [icons.path]);
  }
}

final _key = GlobalKey();

Future<void> _show(
  WidgetTester tester,
  Widget home, {
  Size size = _phone,
  double ratio = 3,
  Brightness brightness = Brightness.light,
}) async {
  tester.view.physicalSize = size * ratio;
  tester.view.devicePixelRatio = ratio;
  await tester.pumpWidget(
    RepaintBoundary(
      key: _key,
      child: MaterialApp(
        debugShowCheckedModeBanner: false,
        theme: buildTheme(brightness),
        home: home,
      ),
    ),
  );
  await _settle(tester);
}

Future<void> _settle(WidgetTester tester, [int ms = 1500]) async {
  for (var i = 0; i < ms ~/ 100; i++) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 20)),
    );
    await tester.pump(const Duration(milliseconds: 100));
  }
}

Future<void> _save(WidgetTester tester, String name, {double ratio = 3}) async {
  await tester.runAsync(() async {
    final boundary =
        _key.currentContext!.findRenderObject()! as RenderRepaintBoundary;
    final image = await boundary.toImage(pixelRatio: ratio);
    final png = await image.toByteData(format: ui.ImageByteFormat.png);
    await File('$_outDir/$name.png').writeAsBytes(png!.buffer.asUint8List());
  });
}

Future<void> _tapTab(WidgetTester tester, String label) async {
  await tester.tap(
    find.descendant(of: find.byType(NavigationBar), matching: find.text(label)),
  );
  await _settle(tester);
}

void main() {
  final skip = _outDir == null || _rustLib == null
      ? 'потрібні STORE_SHOTS і UAUTONOMY_RUST_LIB'
      : null;

  setUpAll(() async {
    if (skip != null) return;
    await _loadFonts();
    await RustLib.init(externalLibrary: ExternalLibrary.open(_rustLib!));
    final dir = await Directory.systemTemp.createTemp('uautonomy-shots');
    await openLibrary(dataDir: dir.path);
  });

  tearDown(() {
    final view =
        TestWidgetsFlutterBinding.instance.platformDispatcher.views.first;
    view.resetPhysicalSize();
    view.resetDevicePixelRatio();
  });

  // Телефон: лише екрани, що не залежать від заліза. «Пакети», «Помічник»
  // і текст «Обміну» знімаються на ПК, бо тест іде на комп'ютері і там
  // видно його пам'ять і підказки для Windows.
  testWidgets('Телефон', skip: skip != null, (tester) async {
    await _show(tester, const HomeScreen());
    await _save(tester, 'phone-1-library');

    await tester.enterText(find.byType(TextField).first, 'кровотеча');
    await _settle(tester);
    await _save(tester, 'phone-2-search');

    await _show(
      tester,
      Scaffold(
        body: CardScreen(card: card(id: 'fa-bleeding')!),
      ),
    );
    await _save(tester, 'phone-3-card');

    final cp = categories().firstWhere((c) => c.slug == 'civil_protection');
    await _show(tester, CategoryScreen(category: cp));
    await _save(tester, 'phone-4-category');

    await _show(
      tester,
      Scaffold(
        body: CardScreen(card: card(id: 'ms-found-object')!),
      ),
    );
    await _save(tester, 'phone-5-card-mine');

    await _show(tester, const HomeScreen(), brightness: Brightness.dark);
    await _save(tester, 'phone-6-library-dark');
  });

  testWidgets('ПК', skip: skip != null, (tester) async {
    Future<void> show(Widget w) =>
        _show(tester, w, size: _desktop, ratio: 1.875);
    Future<void> save(String name) => _save(tester, name, ratio: 1.875);

    await show(const HomeScreen());
    await save('desktop-1-library');

    await show(
      Scaffold(
        body: CardScreen(card: card(id: 'cp-air-raid-home')!),
      ),
    );
    await save('desktop-2-card');

    await show(const HomeScreen());
    await _tapTab(tester, 'Обмін');
    await tester.tap(find.text('Увімкнути обмін'));
    for (
      var i = 0;
      i < 50 && find.text('Вимкнути обмін').evaluate().isEmpty;
      i++
    ) {
      await _settle(tester, 200);
    }
    await _settle(tester);
    await save('desktop-3-share');
    await tester.runAsync(stopSharing);
    await _settle(tester, 300);

    await show(const HomeScreen());
    await _tapTab(tester, 'Пакети');
    // Каталог завантажується з мережі; без неї видно лише встановлене.
    await _settle(tester, 6000);
    await save('desktop-4-packs');
  });
}
