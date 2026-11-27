import 'package:flutter_test/flutter_test.dart';
import 'package:uautonomy/ai/ai_service.dart';
import 'package:uautonomy/src/rust/api/ai.dart';

SourceView _src(int n) {
  return SourceView(
    number: n,
    key: 'card:$n',
    title: 'Картка $n',
    source: 'ДСНС',
    reviewed: false,
  );
}

void main() {
  test('номери посилань у тексті', () {
    final cited = citedNumbers('Вода [1], ліки [2, 3] і ліхтарик [1][4].');
    expect(cited, {1, 2, 3, 4});
    expect(citedNumbers('Без посилань.'), isEmpty);
  });

  test('після відповіді лишаються тільки джерела з посиланнями', () {
    final a = AiAnswer('що покласти в тривожну валізку?', [_src(1), _src(2)]);
    a.text.write('Документи й аптечка [2].');
    expect(a.shownSources.length, 2, reason: 'поки модель пише, видно всі');
    a.done = true;
    expect(a.shownSources.map((s) => s.number), [2]);
  });

  test('без посилань джерел під відповіддю немає', () {
    final a = AiAnswer('як зберегти продукти?', [_src(1), _src(2)]);
    a.text.write('У бібліотеці про це немає.');
    a.done = true;
    expect(a.shownSources, isEmpty);
  });
}
