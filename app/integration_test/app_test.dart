// Наскрізний тест застосунку на справжньому пристрої чи емуляторі.
//
//   flutter test integration_test/app_test.dart -d <пристрій>
//   flutter test integration_test/app_test.dart --dart-define=NETWORK_TESTS=true
//
// Без NETWORK_TESTS усе працює офлайн. З ним тест ще й завантажує карту
// Києва з офіційного каталогу й відкриває її на вкладці «Карти».
import 'dart:io';

import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/main.dart';
import 'package:uautonomy/screens/library_screen.dart';
import 'package:uautonomy/screens/packs_screen.dart';
import 'package:uautonomy/src/rust/api/library.dart';
import 'package:uautonomy/src/rust/api/maps.dart';
import 'package:uautonomy/src/rust/api/packs.dart';
import 'package:uautonomy/src/rust/api/share.dart';
import 'package:uautonomy/src/rust/frb_generated.dart';

const networkTests = bool.fromEnvironment('NETWORK_TESTS');

/// `pumpAndSettle` зависає на нескінченних анімаціях (індикатори
/// завантаження), тому чекаємо фіксований час.
Future<void> settle(WidgetTester tester, [int ms = 800]) async {
  for (var i = 0; i < ms ~/ 100; i++) {
    await tester.pump(const Duration(milliseconds: 100));
  }
}

/// Чекає, доки віджет з'явиться (асинхронні дії на кшталт копіювання APK).
Future<void> waitFor(
  WidgetTester tester,
  Finder finder, {
  int seconds = 60,
}) async {
  for (var i = 0; i < seconds * 10; i++) {
    if (finder.evaluate().isNotEmpty) return;
    await tester.pump(const Duration(milliseconds: 100));
  }
  fail('не дочекалися: $finder');
}

Future<void> openTab(WidgetTester tester, String label) async {
  await tester.tap(
    find.descendant(of: find.byType(NavigationBar), matching: find.text(label)),
  );
  await settle(tester);
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    await RustLib.init();
    final dir = await Directory.systemTemp.createTemp('uautonomy-e2e');
    await openLibrary(dataDir: dir.path);
  });

  testWidgets('Довідка: пошук і картка офлайн', (tester) async {
    await tester.pumpWidget(const UAutonomyApp());
    await settle(tester);

    expect(find.text('Довідка'), findsWidgets);
    expect(categories(), isNotEmpty, reason: 'базовий пакет вбудований');

    await tester.enterText(find.byType(SearchBar), 'кровотеча');
    await settle(tester);
    final firstCard = find.widgetWithIcon(ListTile, Icons.bolt).first;
    expect(firstCard, findsOneWidget, reason: 'пошук знаходить картку');
    await tester.tap(firstCard);
    await settle(tester);
    expect(find.byType(CardScreen), findsOneWidget);
    // Джерело внизу картки: ListView будує лише видиме, тож гортаємо.
    await tester.scrollUntilVisible(
      find.textContaining('Джерело:'),
      300,
      scrollable: find
          .descendant(
            of: find.byType(CardScreen),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    expect(find.textContaining('Джерело:'), findsOneWidget);

    await tester.pageBack();
    await settle(tester);
  });

  testWidgets('Усі вкладки відкриваються без помилок', (tester) async {
    await tester.pumpWidget(const UAutonomyApp());
    await settle(tester);

    for (final tab in ['Помічник', 'Карти', 'Пакети', 'Обмін', 'Довідка']) {
      await openTab(tester, tab);
      expect(tester.takeException(), isNull, reason: 'вкладка $tab');
    }

    await openTab(tester, 'Пакети');
    expect(find.text(officialCatalogUrl), findsOneWidget);
  });

  testWidgets('Обмін: сервер вмикається і вимикається', (tester) async {
    await tester.pumpWidget(const UAutonomyApp());
    await settle(tester);
    await openTab(tester, 'Обмін');

    await tester.tap(find.text('Увімкнути обмін'));
    // На Android спершу копіюється APK для роздачі, це кілька секунд.
    await waitFor(tester, find.text('Вимкнути обмін'));
    final info = sharingStatus();
    debugPrint('Обмін: порт ${info.port}, адреси ${info.urls}');
    expect(info.running, isTrue);
    final client = HttpClient();
    final resp = await (await client.get(
      '127.0.0.1',
      info.port,
      '/api/info',
    )).close();
    expect(resp.statusCode, 200);
    client.close();
    expect(info.urls, isNotEmpty, reason: 'адреса для QR-коду');
    expect(find.textContaining('http://'), findsWidgets);

    await tester.tap(find.text('Вимкнути обмін'));
    await settle(tester, 1000);
    expect(find.text('Увімкнути обмін'), findsOneWidget);
  });

  testWidgets(
    'Карта Києва з офіційного каталогу',
    (tester) async {
      final packs = await fetchCatalog(url: officialCatalogUrl);
      expect(packs.map((p) => p.id), contains('map-kyiv'));
      expect(
        packs.where((p) => p.kind == 'model'),
        isNotEmpty,
        reason: 'у каталозі є модель ШІ',
      );

      await downloadPack(id: 'map-kyiv').drain<void>();
      final kyiv = mapRegions().firstWhere((r) => r.id == 'kyiv');
      expect(kyiv.mapPath, isNotNull);
      final h = await mapHeader(path: kyiv.mapPath!);
      expect(h.vector, isTrue);
      expect(h.bounds.length == 4 || kyiv.bbox.length == 4, isTrue);
      final tile = await mapTile(path: kyiv.mapPath!, z: 10, x: 598, y: 345);
      expect(tile, isNotNull, reason: 'тайл центру Києва');

      await tester.pumpWidget(const UAutonomyApp());
      await settle(tester);
      libraryVersion.value++;
      await openTab(tester, 'Карти');
      await settle(tester, 5000);
      expect(find.textContaining('Не вдалося відкрити карту'), findsNothing);
      expect(find.byType(FlutterMap), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
    skip: !networkTests,
    timeout: const Timeout(Duration(minutes: 10)),
  );
}
