import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:material_ui/material_ui.dart';
import 'package:path_provider/path_provider.dart';

/// Позначка для матеріалів, які ще не перевірив фахівець.
class UnreviewedBadge extends StatelessWidget {
  const UnreviewedBadge({super.key, this.who});

  final String? who;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final label = switch (who) {
      'medic' => 'Не перевірено лікарем',
      'deminer' => 'Ще не перевірено сапером',
      _ => 'Ще не перевірено фахівцем',
    };
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: scheme.tertiaryContainer,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.info_outline, size: 16, color: scheme.onTertiaryContainer),
          const SizedBox(width: 6),
          Flexible(
            child: Text(
              label,
              style: TextStyle(
                color: scheme.onTertiaryContainer,
                fontSize: 13,
                fontWeight: FontWeight.w600,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

const emergencyNote = 'У небезпеці телефонуйте 112 (103 швидка, 101 ДСНС).';

String formatBytes(num bytes) {
  if (bytes >= 1e9) return '${(bytes / 1e9).toStringAsFixed(1)} ГБ';
  if (bytes >= 1e6) return '${(bytes / 1e6).toStringAsFixed(0)} МБ';
  return '${(bytes / 1e3).toStringAsFixed(0)} КБ';
}

void showError(BuildContext context, Object e) {
  ScaffoldMessenger.of(context).showSnackBar(
    SnackBar(content: Text('$e'), duration: const Duration(seconds: 6)),
  );
}

/// Повертає локальний шлях до вибраного файлу. На Android вибір часто
/// дає лише content://, тоді копіюємо потоком у [destDir].
Future<String> localPathFor(PlatformFile f, String destDir) async {
  final p = f.path;
  if (p != null) return p;
  await Directory(destDir).create(recursive: true);
  final out = File('$destDir/${f.name}');
  final sink = out.openWrite();
  try {
    await sink.addStream(f.readAsByteStream());
  } finally {
    await sink.close();
  }
  return out.path;
}

Future<String> appDataDir() async =>
    (await getApplicationSupportDirectory()).path;
