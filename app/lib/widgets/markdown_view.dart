import 'package:flutter/gestures.dart';
import 'package:material_ui/material_ui.dart';

/// Невеликий показ Markdown для статей, сторінок ZIM і відповідей ШІ:
/// заголовки `#`, абзаци, списки `-`/`1.`, таблиці `| … |`, `**жирний**`
/// і посилання `[текст](адреса)`. Без сторонніх пакетів, щоб збірка
/// лишалась простою, а поведінка передбачуваною офлайн.
class MarkdownView extends StatelessWidget {
  const MarkdownView({
    super.key,
    required this.data,
    this.onLink,
    this.selectable = true,
  });

  final String data;

  /// Натиск на посилання (`card:<id>`, `zim:<пакет>/<шлях>` тощо).
  final void Function(String target)? onLink;
  final bool selectable;

  @override
  Widget build(BuildContext context) {
    final blocks = parseMarkdown(data);
    final children = <Widget>[];
    for (final b in blocks) {
      if (children.isNotEmpty) children.add(SizedBox(height: b.gapBefore));
      children.add(_block(context, b));
    }
    final column = Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: children,
    );
    // SelectableText не передає натиски посиланням, а SelectionArea — так.
    return selectable ? SelectionArea(child: column) : column;
  }

  Widget _block(BuildContext context, MdBlock b) {
    final t = Theme.of(context).textTheme;
    final scheme = Theme.of(context).colorScheme;
    switch (b.kind) {
      case MdKind.heading:
        final style = switch (b.level) {
          1 => t.headlineSmall,
          2 => t.titleLarge,
          _ => t.titleMedium,
        };
        return _text(context, b.lines.single, style);
      case MdKind.paragraph:
        return _text(context, b.lines.join('\n'), t.bodyLarge);
      case MdKind.list:
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            for (final (i, item) in b.lines.indexed)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 2),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    SizedBox(
                      width: 28,
                      child: Text(
                        b.ordered ? '${i + 1}.' : '•',
                        style: t.bodyLarge,
                      ),
                    ),
                    Expanded(child: _text(context, item, t.bodyLarge)),
                  ],
                ),
              ),
          ],
        );
      case MdKind.table:
        final rows = b.rows;
        final header = b.hasHeader && rows.isNotEmpty ? rows.first : null;
        final body = header == null ? rows : rows.skip(1);
        return Container(
          decoration: BoxDecoration(
            border: Border.all(color: scheme.outlineVariant),
            borderRadius: BorderRadius.circular(8),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (header != null)
                Container(
                  color: scheme.surfaceContainerHighest,
                  padding: const EdgeInsets.all(8),
                  child: _text(
                    context,
                    header.where((c) => c.isNotEmpty).join(' · '),
                    t.titleSmall,
                  ),
                ),
              for (final r in body)
                Container(
                  decoration: BoxDecoration(
                    border: Border(
                      top: BorderSide(color: scheme.outlineVariant),
                    ),
                  ),
                  padding: const EdgeInsets.all(8),
                  child: _text(
                    context,
                    r.where((c) => c.isNotEmpty).join(' · '),
                    t.bodyMedium,
                  ),
                ),
            ],
          ),
        );
    }
  }

  Widget _text(BuildContext context, String md, TextStyle? style) {
    final link = TextStyle(
      color: Theme.of(context).colorScheme.primary,
      decoration: TextDecoration.underline,
    );
    final spans = inlineSpans(md, linkStyle: link, onLink: onLink);
    return Text.rich(TextSpan(style: style, children: spans));
  }
}

enum MdKind { heading, paragraph, list, table }

class MdBlock {
  MdBlock(this.kind, {this.level = 0, this.ordered = false});

  final MdKind kind;
  final int level;
  final bool ordered;
  final lines = <String>[];
  final rows = <List<String>>[];
  bool hasHeader = false;

  double get gapBefore => kind == MdKind.heading ? 16 : 10;
}

final _ordered = RegExp(r'^\d+[.)]\s+');
final _tableSep = RegExp(r'^\|?[\s:|-]+\|?$');

/// Розбирає Markdown на блоки. HTML-коментарі `<!-- -->` пропускає.
List<MdBlock> parseMarkdown(String src) {
  final text = src.replaceAll(RegExp(r'<!--[\s\S]*?-->'), '');
  final blocks = <MdBlock>[];
  MdBlock? cur;
  void close() {
    if (cur != null) blocks.add(cur!);
    cur = null;
  }

  for (final raw in text.split('\n')) {
    final line = raw.trim();
    if (line.isEmpty) {
      close();
      continue;
    }
    if (line.startsWith('#')) {
      final m = RegExp(r'^(#{1,6})\s+(.*)$').firstMatch(line);
      if (m != null) {
        close();
        blocks.add(
          MdBlock(MdKind.heading, level: m.group(1)!.length.clamp(1, 3))
            ..lines.add(m.group(2)!.replaceAll(RegExp(r'\s*#+$'), '')),
        );
        continue;
      }
    }
    if (line.startsWith('|')) {
      if (cur?.kind != MdKind.table) {
        close();
        cur = MdBlock(MdKind.table);
      }
      if (_tableSep.hasMatch(line) && line.contains('-')) {
        cur!.hasHeader = cur!.rows.length == 1;
        continue;
      }
      var inner = line.substring(1);
      if (inner.endsWith('|') && !inner.endsWith(r'\|')) {
        inner = inner.substring(0, inner.length - 1);
      }
      cur!.rows.add(
        inner.split(RegExp(r'(?<!\\)\|')).map((c) => c.trim()).toList(),
      );
      continue;
    }
    final bullet = line.startsWith('- ') || line.startsWith('* ');
    final num = _ordered.firstMatch(line);
    if (bullet || num != null) {
      final ordered = num != null;
      if (cur?.kind != MdKind.list || cur!.ordered != ordered) {
        close();
        cur = MdBlock(MdKind.list, ordered: ordered);
      }
      cur!.lines.add(ordered ? line.substring(num.end) : line.substring(2));
      continue;
    }
    if (cur?.kind == MdKind.list && raw.startsWith('  ')) {
      // Продовження пункту списку.
      cur!.lines.last = '${cur!.lines.last} $line';
      continue;
    }
    if (cur?.kind != MdKind.paragraph) {
      close();
      cur = MdBlock(MdKind.paragraph);
    }
    cur!.lines.add(line.startsWith('> ') ? line.substring(2) : line);
  }
  close();
  return blocks;
}

/// `**жирний**`, `__жирний__`, `` `код` ``, `[текст](адреса)` і `\`-екранування.
List<InlineSpan> inlineSpans(
  String s, {
  TextStyle? linkStyle,
  void Function(String target)? onLink,
}) {
  final spans = <InlineSpan>[];
  final buf = StringBuffer();
  var bold = false;
  void flush() {
    if (buf.isEmpty) return;
    spans.add(
      TextSpan(
        text: buf.toString(),
        style: bold ? const TextStyle(fontWeight: FontWeight.bold) : null,
      ),
    );
    buf.clear();
  }

  var i = 0;
  while (i < s.length) {
    final c = s[i];
    if (c == r'\' && i + 1 < s.length) {
      buf.write(s[i + 1]);
      i += 2;
      continue;
    }
    if ((s.startsWith('**', i) || s.startsWith('__', i))) {
      flush();
      bold = !bold;
      i += 2;
      continue;
    }
    if (c == '`') {
      i++;
      continue;
    }
    if (c == '[') {
      final close = _findClose(s, i + 1);
      if (close != -1 && close + 1 < s.length && s[close + 1] == '(') {
        final end = linkTargetEnd(s, close + 2);
        if (end != -1) {
          flush();
          final label = s.substring(i + 1, close).replaceAll(r'\', '');
          final target = s.substring(close + 2, end);
          spans.add(
            TextSpan(
              text: label,
              style: onLink == null
                  ? null
                  : bold
                  ? linkStyle?.copyWith(fontWeight: FontWeight.bold)
                  : linkStyle,
              recognizer: onLink == null
                  ? null
                  : (TapGestureRecognizer()..onTap = () => onLink(target)),
            ),
          );
          i = end + 1;
          continue;
        }
      }
    }
    buf.write(c);
    i++;
  }
  flush();
  return spans;
}

int _findClose(String s, int from) {
  for (var i = from; i < s.length; i++) {
    if (s[i] == r'\') {
      i++;
      continue;
    }
    if (s[i] == ']') return i;
    if (s[i] == '[') return -1;
  }
  return -1;
}

/// Прибирає з відповіді ШІ розмітку LaTeX: `$3 \times 12$ л` → `3 × 12 л`.
/// Малі моделі люблять формули, а застосунок їх не рендерить.
String cleanAiText(String s) {
  const symbols = {
    r'\times': '×',
    r'\cdot': '·',
    r'\approx': '≈',
    r'\le': '≤',
    r'\leq': '≤',
    r'\ge': '≥',
    r'\geq': '≥',
    r'\pm': '±',
    r'\div': '÷',
    r'\degree': '°',
    r'\circ': '°',
    r'\%': '%',
    r'\to': '→',
    r'\rightarrow': '→',
    r'\,': ' ',
    r'\;': ' ',
    r'\ ': ' ',
  };
  String math(String m) {
    var x = m;
    x = x.replaceAllMapped(
      RegExp(r'\\[dt]?frac\{([^{}]*)\}\{([^{}]*)\}'),
      (g) => '${g[1]}/${g[2]}',
    );
    x = x.replaceAllMapped(
      RegExp(r'\\(?:text|mathrm|mathbf|textbf|operatorname)\{([^{}]*)\}'),
      (g) => g[1]!,
    );
    x = x.replaceAll(RegExp(r'\^\{?\\circ\}?'), '°');
    final keys = symbols.keys.toList()
      ..sort((a, b) => b.length.compareTo(a.length));
    for (final k in keys) {
      x = x.replaceAllMapped(
        RegExp('${RegExp.escape(k)}(?![a-zA-Z])'),
        (_) => symbols[k]!,
      );
    }
    x = x.replaceAll(RegExp(r'[{}]'), '');
    return x.replaceAll(RegExp(r' {2,}'), ' ').trim();
  }

  var out = s.replaceAllMapped(
    RegExp(r'\$\$([\s\S]+?)\$\$|\\\[([\s\S]+?)\\\]'),
    (g) => math(g[1] ?? g[2]!),
  );
  out = out.replaceAllMapped(RegExp(r'\\\(([\s\S]+?)\\\)'), (g) => math(g[1]!));
  // Інлайн `$…$` лише якщо всередині справді формула (а не ціна «$5»).
  out = out.replaceAllMapped(RegExp(r'\$([^$\n]{1,80}?)\$'), (g) {
    final inner = g[1]!;
    final looksMath =
        inner.contains(r'\') ||
        RegExp(r'^[\d\s.,+\-*/=×^()a-zA-Z]+$').hasMatch(inner);
    return looksMath ? math(inner) : g[0]!;
  });
  return out;
}

/// Кінець адреси посилання з позиції [from]: перша `)` поза вкладеними
/// дужками, бо назви статей Вікіпедії часто мають дужки
/// (`zim:…/Тканина_(біологія)`). -1, якщо адреса не закрита.
int linkTargetEnd(String s, int from) {
  var depth = 0;
  for (var i = from; i < s.length; i++) {
    final c = s[i];
    if (c == '(') {
      depth++;
    } else if (c == ')') {
      if (depth == 0) return i;
      depth--;
    } else if (c == '\n') {
      return -1;
    }
  }
  return -1;
}
