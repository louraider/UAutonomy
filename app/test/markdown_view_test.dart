import 'package:flutter/gestures.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/widgets/markdown_view.dart';

void main() {
  test('Markdown розбирається на блоки', () {
    final b = parseMarkdown(
      '## Запас води\n\nТекст **жирний**.\n<!-- примітка редактора -->\n\n'
      '- один\n- два\n\n| № | Стан |\n|---|---|\n| 19 | [Опіки](card:fa-burns) |',
    );
    expect(b.map((x) => x.kind), [
      MdKind.heading,
      MdKind.paragraph,
      MdKind.list,
      MdKind.table,
    ]);
    expect(b[0].lines.single, 'Запас води');
    expect(b[1].lines.join(' '), isNot(contains('примітка')));
    expect(b[3].hasHeader, isTrue);
    expect(b[3].rows[1], ['19', '[Опіки](card:fa-burns)']);
  });

  test('посилання і жирний текст', () {
    String? tapped;
    final spans = inlineSpans(
      'Див. **[Опіки](card:fa-burns)** і a\\*b',
      onLink: (t) => tapped = t,
    );
    final text = spans.map((s) => (s as TextSpan).text).join();
    expect(text, 'Див. Опіки і a*b');
    expect(tapped, isNull);
  });

  test('адреса посилання з дужками (назви статей Вікіпедії)', () {
    String? tapped;
    final spans = inlineSpans(
      'глибших [тканин](zim:p/Тканина_(біологія)), таких',
      onLink: (t) => tapped = t,
    );
    final text = spans.map((s) => (s as TextSpan).text).join();
    expect(text, 'глибших тканин, таких');
    final link = spans.whereType<TextSpan>().firstWhere(
      (s) => s.text == 'тканин',
    );
    (link.recognizer! as TapGestureRecognizer).onTap!();
    expect(tapped, 'zim:p/Тканина_(біологія)');
  });

  test('LaTeX у відповідях ШІ стає звичайним текстом', () {
    expect(
      cleanAiText(r'Потрібно $3 \times 12$ л води.'),
      'Потрібно 3 × 12 л води.',
    );
    expect(cleanAiText(r'Разом \(9 \cdot 4 = 36\) л'), 'Разом 9 · 4 = 36 л');
    expect(cleanAiText(r'$\frac{1}{2}$ склянки'), '1/2 склянки');
    expect(cleanAiText(r'$$t \approx 60^{\circ}C$$'), 't ≈ 60°C');
    expect(
      cleanAiText('Ціна \$5, а не \$10 за літр'),
      'Ціна \$5, а не \$10 за літр',
    );
  });

  testWidgets('MarkdownView показує текст без розмітки', (tester) async {
    String? link;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: MarkdownView(
            data: '# Вода\n\n**Кип\'ятіння** — 10 хв.\n\n[Картка](card:x)',
            onLink: (t) => link = t,
          ),
        ),
      ),
    );
    expect(find.text('Вода'), findsOneWidget);
    expect(find.textContaining('**'), findsNothing);
    expect(find.textContaining('#'), findsNothing);
    await tester.tapOnText(find.textRange.ofSubstring('Картка'));
    expect(link, 'card:x');
  });
}
