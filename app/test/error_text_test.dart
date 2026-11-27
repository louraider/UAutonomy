import 'package:flutter_test/flutter_test.dart';
import 'package:uautonomy/screens/common.dart';

void main() {
  test('без інтернету людина бачить зрозумілий текст', () {
    const raw =
        'AnyhowException(помилка мережі: io: failed to lookup address '
        'information: No address associated with hostname)';
    expect(errorText(raw), offlineError);
  });

  test('обгортка AnyhowException знімається', () {
    expect(errorText('AnyhowException(пакет пошкоджено)'), 'пакет пошкоджено');
  });

  test('області йдуть за українською абеткою', () {
    final regions = ['Івано-Франківська', 'Київська', 'Автономна', 'Київ']
      ..sort(compareUk);
    expect(regions, ['Автономна', 'Івано-Франківська', 'Київ', 'Київська']);
  });

  test('інші помилки лишаються як є', () {
    expect(errorText('Файл не знайдено'), 'Файл не знайдено');
  });
}
