// Справжня медична енциклопедія (Вікіпедія: медицина з Kiwix) у застосунку:
// пакет ставиться, пошук знаходить статті, сторінка відкривається й
// показується, внутрішні посилання ведуть на інші статті.
//
//   # ZIM уже завантажений (швидко):
//   flutter test integration_test/wikimed_test.dart -d windows \
//     --dart-define=ZIM_PATH=C:\шлях\wikipedia_uk_medicine_maxi_2026-07.zim \
//     --dart-define=OUT_DIR=C:\шлях\до\знімків
//
//   # Як у користувача: каталог → завантаження з Kiwix (~420 МБ):
//   flutter test integration_test/wikimed_test.dart -d windows \
//     --dart-define=NETWORK_TESTS=true
//
// З OUT_DIR зберігає знімки сторінок (PNG). Без ZIM_PATH і NETWORK_TESTS
// тест пропускається.
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/screens/library_screen.dart';
import 'package:uautonomy/screens/packs_screen.dart';
import 'package:uautonomy/src/rust/api/library.dart';
import 'package:uautonomy/src/rust/api/packs.dart';
import 'package:uautonomy/src/rust/frb_generated.dart';
import 'package:uautonomy/widgets/markdown_view.dart';

const zimPath = String.fromEnvironment('ZIM_PATH');
const outDir = String.fromEnvironment('OUT_DIR');
const networkTests = bool.fromEnvironment('NETWORK_TESTS');
const packId = 'zim-wikipedia-uk-medicine';

/// Ставить пакет з уже завантаженого ZIM: підписаний маніфест береться з
/// офіційного каталогу, тож бібліотека перевіряє його як справжній.
Future<void> installFromFile(String dataDir) async {
  final client = HttpClient();
  final req = await client.getUrl(Uri.parse(officialCatalogUrl));
  final resp = await req.close();
  final cat = jsonDecode(
    await resp.transform(utf8.decoder).join(),
  ) as Map<String, dynamic>;
  client.close();
  final pack = (cat['packs'] as List).cast<Map<String, dynamic>>().firstWhere(
    (p) => p['id'] == packId,
  );
  final file = (pack['files'] as List).single as Map<String, dynamic>;
  final dir = Directory('$dataDir/packs/$packId')..createSync(recursive: true);
  final src = File(zimPath);
  expect(
    src.lengthSync(),
    file['size'],
    reason: 'ZIM_PATH не та версія, що в каталозі (${file['path']})',
  );
  // Жорстке посилання замість копії 420 МБ, якщо та сама файлова система.
  final dest = '${dir.path}/${file['path']}';
  final link = await Process.run('cmd', [
    '/c',
    'mklink',
    '/H',
    dest.replaceAll('/', '\\'),
    zimPath,
  ], runInShell: false);
  if (link.exitCode != 0) await src.copy(dest);
  File('${dir.path}/manifest.json').writeAsStringSync(jsonEncode(pack));
}

Future<void> snapshot(WidgetTester tester, GlobalKey key, String name) async {
  if (outDir.isEmpty) return;
  await tester.runAsync(() async {
    final b = key.currentContext!.findRenderObject()! as RenderRepaintBoundary;
    final img = await b.toImage(pixelRatio: 1.5);
    final png = await img.toByteData(format: ui.ImageByteFormat.png);
    Directory(outDir).createSync(recursive: true);
    File('$outDir/$name.png').writeAsBytesSync(png!.buffer.asUint8List());
  });
}

Future<void> showDoc(WidgetTester tester, GlobalKey key, String docKey) async {
  const seed = Color(0xFF0057B7);
  await tester.pumpWidget(
    MaterialApp(
      theme: ThemeData(colorSchemeSeed: seed),
      home: RepaintBoundary(
        key: key,
        child: DocScreen(key: ValueKey(docKey), docKey: docKey),
      ),
    ),
  );
  // document() працює у фоні Rust: чекаємо, доки зникне індикатор.
  for (var i = 0; i < 100; i++) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 100)),
    );
    await tester.pump();
    if (find.byType(CircularProgressIndicator).evaluate().isEmpty) break;
  }
  await tester.pump();
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets(
    'WikiMed: справжні статті відкриваються й показуються',
    (tester) async {
      await RustLib.init();
      final dir = await Directory.systemTemp.createTemp('uautonomy-wikimed');
      await tester.runAsync(() async {
        if (zimPath.isNotEmpty) {
          await installFromFile(dir.path);
        }
        await openLibrary(dataDir: dir.path);
        if (zimPath.isEmpty) {
          await fetchCatalog(url: officialCatalogUrl);
          final sw = Stopwatch()..start();
          await downloadPack(id: packId).drain<void>();
          debugPrint('WIKIMED download_s=${sw.elapsed.inSeconds}');
        }
      });
      final info = libraryInfo();
      debugPrint('WIKIMED zims=${info.zims} warnings=${info.warnings}');
      expect(info.zims, 1);

      final key = GlobalKey();
      var opened = 0;
      for (final q in ['кровотеча', 'опік', 'перелом', 'інсульт', 'діарея']) {
        final hits = search(query: q).where((h) => h.key.startsWith('zim:'));
        debugPrint('WIKIMED q=$q zim_hits=${hits.map((h) => h.title).take(5)}');
        expect(hits, isNotEmpty, reason: 'пошук «$q» не знайшов статей ZIM');
        final hit = hits.first;
        final sw = Stopwatch()..start();
        await showDoc(tester, key, hit.key);
        final d = await tester.runAsync(() => document(key: hit.key));
        expect(d, isNotNull, reason: hit.key);
        debugPrint(
          'WIKIMED open "${d!.title}" ms=${sw.elapsed.inMilliseconds} '
          'chars=${d.body.length} review=${d.reviewRequired}',
        );
        expect(find.text('Не вдалося відкрити'), findsNothing);
        expect(find.byType(MarkdownView), findsOneWidget);
        expect(tester.takeException(), isNull);
        expect(d.body.length, greaterThan(200), reason: d.title);
        expect(d.body, isNot(contains('<div')), reason: d.title);
        expect(d.body, isNot(contains('</')), reason: d.title);
        // Медична енциклопедія має позначку «не перевірено медиком».
        expect(d.reviewRequired, 'medic');
        await snapshot(tester, key, 'wikimed-$q');

        // Перше внутрішнє посилання веде на іншу статтю, що теж відкривається.
        final link = RegExp(r'\]\((zim:[^)]+)\)').firstMatch(d.body)?.group(1);
        if (link != null) {
          final other = await tester.runAsync(() => document(key: link));
          debugPrint('WIKIMED link $link -> ${other?.title}');
          expect(other, isNotNull, reason: 'мертве посилання $link');
          if (opened == 0) {
            await showDoc(tester, key, link);
            expect(find.text('Не вдалося відкрити'), findsNothing);
            expect(tester.takeException(), isNull);
            await snapshot(tester, key, 'wikimed-link');
          }
        }
        opened++;
      }
      expect(opened, 5);
    },
    skip: zimPath.isEmpty && !networkTests,
    timeout: const Timeout(Duration(minutes: 30)),
  );
}
