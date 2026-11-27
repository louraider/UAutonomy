import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/ai/ai_service.dart';
import 'package:uautonomy/screens/settings_screen.dart';
import 'package:uautonomy/src/rust/api/ai.dart';
import 'package:uautonomy/stats/ai_stats.dart';
import 'package:uautonomy/stats/stats.dart';

const _key = 'A-EU-1234567890';

class _FakeServer {
  int? code = 200;
  final bodies = <List<dynamic>>[];

  Future<int?> send(Uri url, String key, String body) async {
    if (code == null) return null;
    bodies.add(jsonDecode(body) as List<dynamic>);
    return code;
  }
}

Stats _stats(_FakeServer server) => Stats(
  key: _key,
  sender: server.send,
  flushDelay: const Duration(hours: 1),
  retryEvery: const Duration(hours: 1),
);

void main() {
  late Directory dir;

  setUp(() => dir = Directory.systemTemp.createTempSync('stats_test'));
  tearDown(() => dir.deleteSync(recursive: true));

  test('адреса за ключем', () {
    expect(
      statsEndpoint(_key).toString(),
      'https://eu.aptabase.com/api/v0/events',
    );
    expect(statsEndpoint(''), isNull);
    expect(statsEndpoint('A-SH-1'), isNull);
    expect(
      statsEndpoint('A-SH-1', host: 'https://s.example.org').toString(),
      'https://s.example.org/api/v0/events',
    );
    expect(statsEndpoint('A-SH-1', host: 'http://s.example.org'), isNull);
  });

  test('без ключа нічого не питає й не записує', () async {
    final s = Stats(key: '');
    await s.init(dir.path);
    expect(s.needsConsent, isFalse);
    s.track('app_open');
    expect(s.queue, isEmpty);
  });

  test('до згоди подій немає, після «Ні» теж', () async {
    final server = _FakeServer();
    final s = _stats(server);
    await s.init(dir.path);
    expect(s.needsConsent, isTrue);
    s.track('app_open');
    expect(s.queue, isEmpty);
    await s.setConsent(false);
    s.track('app_open');
    expect(s.queue, isEmpty);
    expect(server.bodies, isEmpty);
    s.dispose();

    final again = _stats(server);
    await again.init(dir.path);
    expect(again.consent, StatsConsent.no);
    expect(again.needsConsent, isFalse);
    again.dispose();
  });

  test('без мережі події чекають у файлі й ідуть пізніше', () async {
    final server = _FakeServer()..code = null;
    final s = _stats(server);
    await s.init(dir.path);
    await s.setConsent(true);
    s.track('tab_open', {'tab': 'maps'});
    s.track('card_open', {'card': 'fa-bleeding', 'category': 'first_aid'});
    await s.flush();
    expect(s.queue, hasLength(2));
    s.dispose();
    await Future<void>.delayed(const Duration(milliseconds: 50));
    expect(File('${dir.path}/stats_queue.json').existsSync(), isTrue);

    // Новий запуск, мережа з'явилась.
    server.code = 200;
    final next = _stats(server);
    await next.init(dir.path);
    await next.flush();
    expect(next.queue, isEmpty);
    final sent = server.bodies.expand((b) => b).cast<Map<String, dynamic>>();
    expect(sent.map((e) => e['eventName']), ['tab_open', 'card_open']);
    final first = sent.first;
    expect(first['props'], {'tab': 'maps'});
    expect(first['sessionId'], matches(RegExp(r'^\d{18}$')));
    expect((first['systemProps'] as Map).keys, isNot(contains('deviceModel')));
    next.dispose();
  });

  test('відкликана згода стирає чергу', () async {
    final server = _FakeServer()..code = null;
    final s = _stats(server);
    await s.init(dir.path);
    await s.setConsent(true);
    s.track('app_open');
    await Future<void>.delayed(const Duration(milliseconds: 50));
    await s.setConsent(false);
    expect(s.queue, isEmpty);
    expect(File('${dir.path}/stats_queue.json').existsSync(), isFalse);
    s.dispose();
  });

  test('хибну пачку сервер відкидає, а збій сервера повторюємо', () async {
    final server = _FakeServer()..code = 503;
    final s = _stats(server);
    await s.init(dir.path);
    await s.setConsent(true);
    s.track('app_open');
    await s.flush();
    expect(s.queue, hasLength(1));
    server.code = 400;
    await s.flush();
    expect(s.queue, isEmpty);
    s.dispose();
  });

  test('результат запитання до помічника без тексту', () {
    const src = SourceView(
      number: 1,
      key: 'card:x',
      title: 'Вода',
      source: 'ДСНС',
      reviewed: true,
    );
    final ok = AiAnswer('Як очистити воду?', const [src])
      ..text.write('1. Закип\'ятіть воду [1].');
    expect(answerOutcome(ok), 'answered');
    final none = AiAnswer('Що таке квазар?', const [src])
      ..text.write('У бібліотеці цього немає.');
    expect(answerOutcome(none), 'not_in_library');
    expect(answerOutcome(AiAnswer('?', const [])), 'no_sources');
    final noModel = AiAnswer('?', const [])..error = 'Модель не завантажено';
    expect(answerOutcome(noModel), 'no_model');
    final route = AiAnswer('?', const [])
      ..route = const RouteView(
        mode: 'cards',
        topic: 'medical',
        message: '',
        cards: [],
      );
    expect(answerOutcome(route), 'no_card');
  });

  testWidgets('запит згоди: «Так» основна кнопка, «Ні» поруч', (tester) async {
    final server = _FakeServer();
    final s = _stats(server);
    await tester.runAsync(() => s.init(dir.path));
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => TextButton(
            onPressed: () => askStatsConsent(context, stats: s),
            child: const Text('go'),
          ),
        ),
      ),
    );
    await tester.tap(find.text('go'));
    await tester.pumpAndSettle();
    expect(find.text('Допоможете зробити «Автономію» кращою?'), findsOneWidget);
    expect(find.widgetWithText(FilledButton, 'Так'), findsOneWidget);
    expect(find.widgetWithText(TextButton, 'Ні'), findsOneWidget);
    await tester.runAsync(() async {
      await tester.tap(find.text('Ні'));
      await Future<void>.delayed(const Duration(milliseconds: 100));
    });
    await tester.pumpAndSettle();
    expect(s.consent, StatsConsent.no);
    s.dispose();
  });

  testWidgets('у налаштуваннях статистику можна вимкнути', (tester) async {
    final server = _FakeServer();
    final s = _stats(server);
    await tester.runAsync(() async {
      await s.init(dir.path);
      await s.setConsent(true);
    });
    await tester.pumpWidget(MaterialApp(home: SettingsScreen(stats: s)));
    expect(find.text('Надіслати відгук'), findsOneWidget);
    final sw = find.byType(SwitchListTile);
    expect(tester.widget<SwitchListTile>(sw).value, isTrue);
    await tester.tap(sw);
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 100)),
    );
    await tester.pump();
    expect(s.enabled, isFalse);
    expect(tester.widget<SwitchListTile>(sw).value, isFalse);
    s.dispose();
  });
}
