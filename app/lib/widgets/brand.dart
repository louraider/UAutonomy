import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/theme.dart';

/// Знак «Автономії»: синій квадрат, горизонталі й жовта мітка «ви тут».
/// Чим менший знак, тим менше горизонталей, щоб він не розпливався.
class BrandMark extends StatelessWidget {
  const BrandMark({super.key, this.size = 32});

  final double size;

  @override
  Widget build(BuildContext context) => SizedBox.square(
    dimension: size,
    child: CustomPaint(painter: _MarkPainter(size)),
  );
}

class _MarkPainter extends CustomPainter {
  _MarkPainter(this.px);

  final double px;

  static Path _ring(List<double> p) {
    // p: start x,y, далі чотири кубічні сегменти по 6 чисел.
    final path = Path()..moveTo(p[0], p[1]);
    for (var i = 2; i < p.length; i += 6) {
      path.cubicTo(p[i], p[i + 1], p[i + 2], p[i + 3], p[i + 4], p[i + 5]);
    }
    return path..close();
  }

  static final _outer = _ring([
    26, 64, 26, 42, 44, 26, 66, 26, 88, 26, 98, 40, 98, 56, //
    98, 72, 84, 92, 62, 92, 40, 92, 26, 84, 26, 64,
  ]);
  static final _middle = _ring([
    40, 62, 40, 49, 51, 39, 66, 39, 81, 39, 87, 48, 87, 58, //
    87, 68, 76, 80, 63, 80, 50, 80, 40, 74, 40, 62,
  ]);
  static final _inner = _ring([
    53, 60, 53, 53, 59, 49, 66, 49, 73, 49, 76, 53, 76, 58, //
    76, 63, 71, 69, 64, 69, 57, 69, 53, 66, 53, 60,
  ]);

  @override
  void paint(Canvas canvas, Size size) {
    canvas.scale(size.width / 120);
    canvas.drawRRect(
      RRect.fromRectAndRadius(
        const Rect.fromLTWH(0, 0, 120, 120),
        const Radius.circular(28),
      ),
      Paint()..color = BrandColors.light.blue,
    );
    final rings = px >= 64
        ? [_outer, _middle, _inner]
        : px >= 40
        ? [_outer, _middle]
        : [_outer];
    final stroke = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = px >= 64 ? 4 : (px >= 40 ? 5 : 8)
      ..color = const Color(0x99FFFFFF);
    for (final r in rings) {
      canvas.drawPath(r, stroke);
    }
    canvas.drawCircle(
      const Offset(65, 59),
      px >= 64 ? 7 : (px >= 40 ? 9 : 14),
      Paint()..color = BrandColors.light.marker,
    );
  }

  @override
  bool shouldRepaint(_MarkPainter old) => old.px != px;
}

/// Заголовок вкладки: велика назва і, за потреби, дії праворуч.
class TabHeader extends StatelessWidget {
  const TabHeader({
    super.key,
    required this.title,
    this.trailing = const [],
    this.showMark = false,
  });

  final String title;
  final List<Widget> trailing;
  final bool showMark;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 16, 8, 8),
      child: Row(
        children: [
          Expanded(
            child: Text(
              title,
              style: Theme.of(context).textTheme.headlineMedium,
            ),
          ),
          ...trailing,
          if (showMark)
            const Padding(
              padding: EdgeInsets.only(left: 8, right: 8),
              child: BrandMark(size: 32),
            ),
        ],
      ),
    );
  }
}

/// Жовта смуга з номерами екстрених служб.
class SosBar extends StatelessWidget {
  const SosBar({super.key});

  @override
  Widget build(BuildContext context) {
    final b = BrandColors.of(context);
    return Semantics(
      label: 'У небезпеці телефонуйте 112, швидка 103, ДСНС 101',
      excludeSemantics: true,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
        decoration: BoxDecoration(
          color: b.marker,
          borderRadius: BorderRadius.circular(12),
        ),
        child: Row(
          children: [
            Icon(Icons.call, size: 20, color: b.onMarker),
            const SizedBox(width: 10),
            Expanded(
              child: Text.rich(
                TextSpan(
                  style: TextStyle(
                    color: b.onMarker,
                    fontSize: 15,
                    fontWeight: FontWeight.w600,
                  ),
                  children: const [
                    TextSpan(text: 'У небезпеці: '),
                    TextSpan(
                      text: '112',
                      style: TextStyle(fontWeight: FontWeight.w800),
                    ),
                    TextSpan(text: ' · 103 · 101'),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// Обмежує ширину вмісту на великих екранах (Windows), щоб рядки не
/// розтягувались на весь монітор.
class ContentWidth extends StatelessWidget {
  const ContentWidth({super.key, required this.child, this.maxWidth = 760});

  final Widget child;
  final double maxWidth;

  @override
  Widget build(BuildContext context) => Align(
    alignment: Alignment.topCenter,
    child: ConstrainedBox(
      constraints: BoxConstraints(maxWidth: maxWidth),
      child: child,
    ),
  );
}
