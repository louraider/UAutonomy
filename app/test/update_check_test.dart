import 'package:flutter_test/flutter_test.dart';
import 'package:uautonomy/update/update_check.dart';

void main() {
  const url = 'https://uautonomy.app/android';

  test('новіша збірка — є оновлення', () {
    final u = parseUpdate(
      '{"build": 80, "date": "2026-11-26", "commit": "abc1234"}',
      build: 70,
      platformUrl: url,
    );
    expect(u, isNotNull);
    expect(u!.build, 80);
    expect(u.date, '2026-11-26');
    expect(u.url, url);
  });

  test('та сама чи старіша збірка — тиша', () {
    expect(parseUpdate('{"build": 70}', build: 70, platformUrl: url), isNull);
    expect(parseUpdate('{"build": 60}', build: 70, platformUrl: url), isNull);
  });

  test('відкладене оновлення не показуємо знову, наступне — показуємо', () {
    expect(
      parseUpdate('{"build": 80}', build: 70, platformUrl: url, dismissed: 80),
      isNull,
    );
    expect(
      parseUpdate('{"build": 81}', build: 70, platformUrl: url, dismissed: 80),
      isNotNull,
    );
  });

  test('зіпсований чи чужий файл — тиша', () {
    for (final body in ['', 'not json', '[]', '{"build": "80"}', '<html>']) {
      expect(parseUpdate(body, build: 1, platformUrl: url), isNull);
    }
  });

  test('локальна збірка нових версій не шукає', () {
    expect(currentBuild, 0);
    expect(updateCheckSupported, isFalse);
  });
}
