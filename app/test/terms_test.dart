import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:uautonomy/legal/terms.dart';
import 'package:uautonomy/widgets/markdown_view.dart';

void main() {
  final terms = File(termsAsset).readAsStringSync();

  test('умови містять обмеження Gemma і медичне застереження', () {
    expect(terms, contains('Gemma Prohibited Use Policy'));
    expect(terms, contains('ai.google.dev/gemma/terms'));
    expect(terms, contains('Не перевірено лікарем'));
    expect(terms, contains('112'));
  });

  test('сайт показує ту саму редакцію умов', () {
    final site = File('../site/terms.html').readAsStringSync();
    expect(site, contains('data-terms-version="$termsVersion"'));
    for (final b in parseMarkdown(terms)) {
      if (b.kind == MdKind.heading) {
        expect(site, contains(b.lines.single), reason: 'немає розділу');
      }
    }
  });

  test('тексти Gemma в застосунку збігаються з пакетом моделі', () {
    for (final name in [
      'GEMMA_TERMS_OF_USE.txt',
      'GEMMA_PROHIBITED_USE_POLICY.txt',
    ]) {
      expect(
        File('assets/legal/$name').readAsStringSync(),
        File('../tools/packs/licenses/gemma/$name').readAsStringSync(),
        reason: name,
      );
    }
  });
}
