// Наскрізний тест застосунку на справжньому пристрої чи емуляторі.
//
//   flutter test integration_test/app_test.dart -d <пристрій>
//   flutter test integration_test/app_test.dart --dart-define=NETWORK_TESTS=true
//
// Без NETWORK_TESTS усе працює офлайн. З ним тест ще й завантажує карту
// Києва з офіційного каталогу й відкриває її на вкладці «Карти».
import 'dart:io';

import 'package:flutter/gestures.dart';

import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/ai/ai_service.dart';
import 'package:uautonomy/main.dart';
import 'package:uautonomy/screens/library_screen.dart';
import 'package:uautonomy/screens/packs_screen.dart';
import 'package:uautonomy/screens/share_screen.dart';
import 'package:uautonomy/src/rust/api/device.dart';
import 'package:uautonomy/src/rust/api/library.dart';
import 'package:uautonomy/src/rust/api/maps.dart';
import 'package:uautonomy/src/rust/api/packs.dart';
import 'package:uautonomy/src/rust/api/share.dart';
import 'package:uautonomy/src/rust/frb_generated.dart';
import 'package:vector_map_tiles/vector_map_tiles.dart';

const networkTests = bool.fromEnvironment('NETWORK_TESTS');

/// Запуск llama.cpp на крихітній моделі (stories260K, ~1 МБ). Вмикається
/// окремо (CI на симуляторі iOS), бо модель завантажується з Hugging Face.
const aiTests = bool.fromEnvironment('AI_TESTS');
const tinyModelUrl =
    'https://huggingface.co/ggml-org/models/resolve/main/tinyllamas/stories260K.gguf';

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
    final shareList = find
        .descendant(
          of: find.byType(ShareScreen),
          matching: find.byType(Scrollable),
        )
        .first;
    // Адреса під QR-кодом (SelectableText); ListView будує лише видиме.
    // Finder.first кидає на порожньому, тож шукаємо рівно першу адресу.
    final address = find.byWidgetPredicate(
      (w) => w is SelectableText && w.data == info.urls.first,
    );
    await tester.scrollUntilVisible(address, 200, scrollable: shareList);
    expect(address, findsOneWidget);
    await tester.scrollUntilVisible(
      find.text('Вимкнути обмін'),
      -200,
      scrollable: shareList,
    );

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
      expect(
        find.text('Київ'),
        findsWidgets,
        reason: 'відкрито встановлену область, а не першу в списку',
      );

      // Карта реагує на керування: раніше CameraConstraint.contain на
      // широкому вікні відкидав будь-який рух і масштаб.
      MapCamera camera() =>
          MapCamera.of(tester.element(find.byType(VectorTileLayer)));
      final before = camera();
      final mid = tester.getCenter(find.byType(FlutterMap));
      await tester.sendEventToBinding(
        PointerScrollEvent(position: mid, scrollDelta: const Offset(0, -400)),
      );
      await settle(tester, 1500);
      expect(camera().zoom, greaterThan(before.zoom), reason: 'колесо миші');
      final zoomed = camera();
      await tester.dragFrom(mid, const Offset(150, 80));
      await settle(tester, 1500);
      expect(
        camera().center,
        isNot(equals(zoomed.center)),
        reason: 'перетягування',
      );
      expect(tester.takeException(), isNull);
    },
    skip: !networkTests,
    timeout: const Timeout(Duration(minutes: 10)),
  );

  testWidgets(
    'llama.cpp завантажує модель GGUF',
    (tester) async {
      final dir = await Directory.systemTemp.createTemp('uautonomy-ai');
      final file = File('${dir.path}/stories260K.gguf');
      final client = HttpClient();
      final resp = await (await client.getUrl(Uri.parse(tinyModelUrl))).close();
      expect(resp.statusCode, 200);
      await resp.pipe(file.openWrite());
      client.close();

      final model = await importModelFile(path: file.path);
      final ai = AiService();
      await ai.load(model);
      expect(ai.isLoaded, isTrue);
      // На iPhone llama.cpp має працювати через Metal; на симуляторі
      // залежить від Mac, тож лише друкуємо, що вибрано.
      debugPrint('llama.cpp: ${ai.backendName}, шарів на GPU: ${ai.gpuLayers}');
      expect(ai.backendName, isNotNull);
      await ai.unload();
    },
    skip: !aiTests,
    timeout: const Timeout(Duration(minutes: 5)),
  );
}
