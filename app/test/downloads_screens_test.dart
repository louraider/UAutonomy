// Екран першого запуску й картка «Усе потрібне» на підробленому каталозі.
// З SHOTS=/тека ще й зберігає знімки екранів для перевірки на око.
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/downloads/download_manager.dart';
import 'package:uautonomy/downloads/download_widgets.dart';
import 'package:uautonomy/screens/welcome_screen.dart';
import 'package:uautonomy/theme.dart';

import 'fake_packs.dart';

final _shots = Platform.environment['SHOTS'];
final _key = GlobalKey();

Future<void> _fonts() async {
  Future<void> family(String name, List<String> files) async {
    final loader = FontLoader(name);
    for (final f in files) {
      loader.addFont(File(f).readAsBytes().then(ByteData.sublistView));
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
  final root = Platform.environment['FLUTTER_ROOT'];
  final icons = File(
    '$root/bin/cache/artifacts/material_fonts/MaterialIcons-Regular.otf',
  );
  if (root != null && icons.existsSync()) {
    await family('MaterialIcons', [icons.path]);
  }
}

Future<void> _pump(WidgetTester tester, Widget home) async {
  tester.view.physicalSize = const Size(390, 900) * 2;
  tester.view.devicePixelRatio = 2;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    RepaintBoundary(
      key: _key,
      child: MaterialApp(
        debugShowCheckedModeBanner: false,
        theme: buildTheme(Brightness.light),
        home: home,
      ),
    ),
  );
  await tester.pump();
  await tester.pump();
}

Future<void> _save(WidgetTester tester, String name) async {
  if (_shots == null) return;
  await tester.runAsync(() async {
    final b = _key.currentContext!.findRenderObject()! as RenderRepaintBoundary;
    final img = await b.toImage(pixelRatio: 2);
    final png = await img.toByteData(format: ui.ImageByteFormat.png);
    await File('$_shots/$name.png').writeAsBytes(png!.buffer.asUint8List());
  });
}

void main() {
  setUpAll(() async {
    if (_shots != null) await _fonts();
  });

  testWidgets('Перший запуск: область, набір і одна кнопка', (tester) async {
    final dir = Directory.systemTemp.createTempSync('uaut-welcome');
    addTearDown(() => dir.deleteSync(recursive: true));
    final b = FakeBackend(sampleCatalog);
    final m = DownloadManager(b);
    bool? started;
    await _pump(
      tester,
      WelcomeScreen(
        manager: m,
        prefs: DownloadPrefs(dir.path),
        onDone: (s) => started = s,
      ),
    );
    expect(find.textContaining('Вікіпедія: медицина'), findsOneWidget);
    expect(find.textContaining('Gemma 4 E2B'), findsOneWidget);
    expect(find.textContaining('Спершу виберіть область'), findsOneWidget);
    await _save(tester, 'welcome-1');

    await tester.tap(find.byType(DropdownButtonFormField<String>));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Київська').last);
    await tester.pumpAndSettle();
    expect(DownloadPrefs(dir.path).region, 'kyivska');
    expect(
      find.text('Карта: Київ'),
      findsOneWidget,
      reason: 'Київ разом з областю',
    );
    // 108 + 21 + 423 МБ + 3,1 ГБ
    expect(find.text('Завантажити все · 3.7 ГБ'), findsOneWidget);
    await _save(tester, 'welcome-2-region');

    // Без моделі ШІ.
    final gemma = find.descendant(
      of: find.widgetWithText(
        CheckboxListTile,
        'Помічник ШІ: Gemma 4 E2B (телефон)',
      ),
      matching: find.byType(Checkbox),
    );
    await tester.ensureVisible(gemma);
    await tester.pumpAndSettle();
    await tester.tap(gemma);
    await tester.pump();
    expect(find.text('Завантажити все · 552 МБ'), findsOneWidget);

    await tester.ensureVisible(find.text('Завантажити все · 552 МБ'));
    await tester.tap(find.text('Завантажити все · 552 МБ'));
    await tester.pump();
    expect(started, isTrue);
    expect(m.queue, ['map-kyivska', 'map-kyiv', recommendedEncyclopediaId]);
    expect(b.started, ['map-kyivska']);
  });

  testWidgets('Черга: прогрес, пауза, помилка без мережі', (tester) async {
    final b = FakeBackend(sampleCatalog);
    final m = DownloadManager(b);
    await m.ensureCatalog();
    m.enqueue(['map-lvivska', recommendedEncyclopediaId]);
    await _pump(
      tester,
      Scaffold(
        body: ListView(
          children: [
            DownloadQueueCard(manager: m),
            RecommendedSetCard(manager: m, prefs: null),
          ],
        ),
      ),
    );
    b.progress('map-lvivska', 30000000, 69814912);
    await tester.pump();
    expect(find.textContaining('43% · 30 МБ з 70 МБ'), findsWidgets);
    expect(find.textContaining('Далі в черзі: 1'), findsOneWidget);
    await _save(tester, 'queue-1');

    await b.fail(
      'map-lvivska',
      'AnyhowException(io: connection reset by peer)',
    );
    await tester.pump();
    await tester.pump();
    expect(find.text('Завантаження призупинено'), findsOneWidget);
    expect(find.textContaining('Немає зв’язку'), findsWidgets);
    await _save(tester, 'queue-2-offline');

    await tester.ensureVisible(find.text('Продовжити'));
    await tester.tap(find.text('Продовжити'));
    await tester.pump();
    expect(b.started, ['map-lvivska', 'map-lvivska']);
  });

  testWidgets('Без інтернету каталог пояснює, що робити', (tester) async {
    final b = FakeBackend(sampleCatalog)
      ..catalogError = 'AnyhowException(dns error: failed to lookup address)';
    final m = DownloadManager(b);
    await _pump(
      tester,
      Scaffold(body: RecommendedSetCard(manager: m, prefs: null)),
    );
    expect(find.textContaining('Немає зв’язку'), findsOneWidget);
    expect(find.textContaining('вкладка «Обмін»'), findsOneWidget);
    expect(find.text('Спробувати ще раз'), findsOneWidget);
    await _save(tester, 'offline');
    b.catalogError = null;
    await tester.tap(find.text('Спробувати ще раз'));
    await tester.pump();
    await tester.pump();
    expect(find.text('Ваша область (для карти)'), findsOneWidget);
  });
}
