import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:uautonomy/downloads/download_manager.dart';

import 'fake_packs.dart';

Future<void> tick() => Future<void>.delayed(Duration.zero);

void main() {
  final catalog = [
    pack('map-kyivska', 'map', region: 'kyivska', title: 'Карта: Київська'),
    pack('map-kyiv', 'map', region: 'kyiv', title: 'Карта: Київ'),
    pack('map-lvivska', 'map', region: 'lvivska', title: 'Карта: Львівська'),
    pack(recommendedEncyclopediaId, 'content', size: 400),
    pack('zim-wikipedia-uk-top', 'content', size: 3000),
    pack('model-e2b', 'model', modelId: 'gemma-e2b', size: 3000),
    pack('model-12b', 'model', modelId: 'mamay-12b', size: 7000),
  ];

  test('каталог відкривається сам і дає рекомендований набір', () async {
    final b = FakeBackend(catalog);
    final m = DownloadManager(b);
    await m.ensureCatalog();
    expect(m.catalogStatus, CatalogStatus.ready);
    expect(m.recommendedModelId, 'gemma-e2b');
    expect(m.recommendedSet(regionId: 'lvivska').map((p) => p.id), [
      'map-lvivska',
      recommendedEncyclopediaId,
      'model-e2b',
    ]);
    // Для Київської області додаємо й Київ.
    expect(
      m.recommendedSet(regionId: 'kyivska').map((p) => p.id),
      containsAllInOrder(['map-kyivska', 'map-kyiv']),
    );
    expect(m.mapRegions.map((r) => r.$2), ['Київ', 'Київська', 'Львівська']);
  });

  test('без інтернету показуємо зрозумілу помилку, не сирий текст', () async {
    final b = FakeBackend(catalog)
      ..catalogError =
          'AnyhowException(помилка мережі: io: failed to lookup address information)';
    final m = DownloadManager(b);
    await m.ensureCatalog();
    expect(m.catalogStatus, CatalogStatus.failed);
    expect(m.catalogError, contains('Немає зв’язку'));
    b.catalogError = null;
    await m.ensureCatalog();
    expect(m.catalogStatus, CatalogStatus.ready);
  });

  test('черга качає по одному і пропускає встановлене', () async {
    final b = FakeBackend(catalog);
    final m = DownloadManager(b);
    var installedCalls = 0;
    m.onInstalled = () => installedCalls++;
    await m.ensureCatalog();
    m.enqueue(['map-lvivska', recommendedEncyclopediaId, 'map-lvivska']);
    await tick();
    expect(m.queue, ['map-lvivska', recommendedEncyclopediaId]);
    expect(b.started, ['map-lvivska']);
    expect(m.stateOf('map-lvivska'), PackState.downloading);
    expect(m.stateOf(recommendedEncyclopediaId), PackState.queued);

    b.progress('map-lvivska', 500, 1000);
    await tick();
    expect(m.progress?.done, BigInt.from(500));

    await b.finish('map-lvivska');
    await tick();
    expect(m.stateOf('map-lvivska'), PackState.installed);
    expect(b.started, ['map-lvivska', recommendedEncyclopediaId]);
    await b.finish(recommendedEncyclopediaId);
    await tick();
    await tick();
    expect(m.queue, isEmpty);
    expect(m.busy, isFalse);
    expect(installedCalls, 2);

    m.enqueue(['map-lvivska']);
    await tick();
    expect(b.started.length, 2, reason: 'встановлене вдруге не качаємо');
  });

  test('пауза і продовження з того ж пакета', () async {
    final b = FakeBackend(catalog);
    final m = DownloadManager(b);
    await m.ensureCatalog();
    m.enqueue(['model-e2b', 'map-kyiv']);
    await tick();
    b.progress('model-e2b', 100, 3000);
    await tick();
    m.pause();
    await tick();
    await tick();
    expect(m.paused, isTrue);
    expect(m.error, isNull, reason: 'пауза не є помилкою');
    expect(m.stateOf('model-e2b'), PackState.paused);
    expect(m.queue, ['model-e2b', 'map-kyiv']);

    m.resume();
    await tick();
    expect(b.started, ['model-e2b', 'model-e2b']);
    await b.finish('model-e2b');
    await tick();
    expect(b.started.last, 'map-kyiv');
  });

  test('обрив мережі зупиняє чергу, «Продовжити» качає далі', () async {
    final b = FakeBackend(catalog);
    final m = DownloadManager(b);
    await m.ensureCatalog();
    m.enqueue(['map-kyiv', 'map-lvivska']);
    await tick();
    await b.fail(
      'map-kyiv',
      'AnyhowException(помилка мережі: connection reset)',
    );
    await tick();
    await tick();
    expect(m.stateOf('map-kyiv'), PackState.failed);
    expect(m.error, contains('Немає зв’язку'));
    expect(b.started, ['map-kyiv'], reason: 'без мережі далі не йдемо');

    m.resume();
    await tick();
    expect(m.error, isNull);
    expect(b.started, ['map-kyiv', 'map-kyiv']);
  });

  test('прибрати поточний пакет з черги — черга йде далі', () async {
    final b = FakeBackend(catalog);
    final m = DownloadManager(b);
    await m.ensureCatalog();
    m.enqueue(['model-12b', 'map-kyiv']);
    await tick();
    m.remove('model-12b');
    await tick();
    await tick();
    expect(m.paused, isFalse);
    expect(m.queue, ['map-kyiv']);
    expect(b.started, ['model-12b', 'map-kyiv']);
    expect(m.stateOf('model-12b'), PackState.available);
  });

  test('налаштування: область і перший запуск', () async {
    final dir = await Directory.systemTemp.createTemp('uaut-prefs');
    final p = DownloadPrefs(dir.path);
    expect(p.region, isNull);
    expect(p.welcomeDone, isFalse);
    p.region = 'lvivska';
    p.markWelcomeDone();
    final q = DownloadPrefs(dir.path);
    expect(q.region, 'lvivska');
    expect(q.welcomeDone, isTrue);
    await dir.delete(recursive: true);
  });
}
