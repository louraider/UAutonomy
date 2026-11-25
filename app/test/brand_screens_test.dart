import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/foundation.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/screens/library_screen.dart';
import 'package:uautonomy/src/rust/api/library.dart';
import 'package:uautonomy/theme.dart';

/// Вигляд головних екранів у світлій і темній темі на телефоні й на
/// широкому вікні: нічого не вилазить за межі. З SCREENSHOTS=<тека>
/// ще й зберігає PNG для перегляду дизайну.
const _card = CardView(
  id: 'fa-bleeding',
  category: 'first_aid',
  title: 'Сильна кровотеча з рани на руці чи нозі',
  summary: 'Тисніть на рану. Якщо кров б\'є струменем, накладіть турнікет.',
  steps: [
    'Подбайте про власну безпеку і викличте 103 або 112.',
    'Притисніть рану чистою тканиною і тримайте, не відпускаючи.',
    'Якщо кров не зупиняється, накладіть турнікет вище рани.',
  ],
  warnings: ['Не знімайте турнікет самі.', 'Не промивайте глибоку рану.'],
  source: 'Наказ МОЗ № 441',
  reviewed: false,
  reviewRequired: 'medic',
);

List<CategoryView> _sample() => const [
  CategoryView(
    slug: 'first_aid',
    title: 'Перша допомога',
    cards: [_card, _card, _card],
  ),
  CategoryView(
    slug: 'civil_protection',
    title: 'Тривога й укриття',
    cards: [_card, _card],
  ),
  CategoryView(slug: 'mine_safety', title: 'Мінна безпека', cards: [_card]),
  CategoryView(
    slug: 'household',
    title: 'Блекаут, вода, тепло',
    cards: [_card, _card, _card, _card],
  ),
];

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

Future<void> _shot(
  WidgetTester tester,
  Widget screen,
  String name,
  Brightness brightness,
  Size size,
) async {
  final key = GlobalKey();
  tester.view.physicalSize = size * 2;
  tester.view.devicePixelRatio = 2;
  await tester.pumpWidget(
    RepaintBoundary(
      key: key,
      child: MaterialApp(
        debugShowCheckedModeBanner: false,
        theme: buildTheme(brightness),
        home: screen,
      ),
    ),
  );
  await tester.pumpAndSettle();
  expect(tester.takeException(), isNull, reason: name);
  final dir = Platform.environment['SCREENSHOTS'];
  if (dir == null) return;
  await tester.runAsync(() async {
    final boundary =
        key.currentContext!.findRenderObject()! as RenderRepaintBoundary;
    final image = await boundary.toImage(pixelRatio: 2);
    final png = await image.toByteData(format: ui.ImageByteFormat.png);
    final mode = brightness == Brightness.dark ? 'dark' : 'light';
    await File('$dir/$name-$mode-${size.width.toInt()}.png')
        .writeAsBytes(png!.buffer.asUint8List());
  });
}

void main() {
  setUpAll(_loadFonts);
  tearDown(() {
    final view =
        TestWidgetsFlutterBinding.instance.platformDispatcher.views.first;
    view.resetPhysicalSize();
    view.resetDevicePixelRatio();
  });

  for (final brightness in Brightness.values) {
    for (final size in const [Size(390, 844), Size(1280, 800)]) {
      final label = '${brightness.name} ${size.width.toInt()}';
      testWidgets('Довідка: $label', (tester) async {
        await _shot(
          tester,
          Scaffold(
            body: SafeArea(
              child: LibraryScreen(
                version: ValueNotifier(0),
                loadCategories: _sample,
              ),
            ),
            bottomNavigationBar: NavigationBar(
              destinations: const [
                NavigationDestination(
                  icon: Icon(Icons.menu_book_outlined),
                  selectedIcon: Icon(Icons.menu_book),
                  label: 'Довідка',
                ),
                NavigationDestination(
                  icon: Icon(Icons.forum_outlined),
                  label: 'Помічник',
                ),
                NavigationDestination(
                  icon: Icon(Icons.map_outlined),
                  label: 'Карти',
                ),
                NavigationDestination(
                  icon: Icon(Icons.download_outlined),
                  label: 'Пакети',
                ),
                NavigationDestination(
                  icon: Icon(Icons.wifi_tethering),
                  label: 'Обмін',
                ),
              ],
            ),
          ),
          'library',
          brightness,
          size,
        );
      });
      testWidgets('Картка: $label', (tester) async {
        await _shot(
          tester,
          const CardScreen(card: _card),
          'card',
          brightness,
          size,
        );
      });
    }
  }
}
