import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/distribution.dart';
import 'package:uautonomy/stats/stats.dart';
import 'package:url_launcher/url_launcher.dart';

/// Скарга на відповідь помічника: людина бачить, що саме буде надіслано,
/// і сама обирає, листом чи через GitHub. Застосунок нічого не надсилає
/// без її дії. Цього вимагає й Google Play для застосунків із генеративним ШІ.
Future<void> showReportSheet(
  BuildContext context, {
  required String question,
  required String answer,
}) {
  track('ai_report_open');
  final report =
      'Запитання:\n$question\n\nВідповідь помічника:\n$answer\n\n'
      'Що не так:\n';
  return showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    showDragHandle: true,
    builder: (ctx) => _ReportSheet(report: report),
  );
}

class _ReportSheet extends StatelessWidget {
  const _ReportSheet({required this.report});

  final String report;

  Future<void> _open(BuildContext context, Uri uri) async {
    var ok = false;
    try {
      ok = await launchUrl(uri, mode: LaunchMode.externalApplication);
    } catch (_) {}
    if (!context.mounted) return;
    if (!ok) {
      await Clipboard.setData(ClipboardData(text: report));
      if (!context.mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text(
            'Не вдалося відкрити. Текст скарги скопійовано: надішліть його на $supportEmail.',
          ),
        ),
      );
    }
    Navigator.of(context).pop();
  }

  @override
  Widget build(BuildContext context) {
    final t = Theme.of(context).textTheme;
    final mail = Uri(
      scheme: 'mailto',
      path: supportEmail,
      query: _query({
        'subject': 'Автономія: погана відповідь помічника',
        'body': report,
      }),
    );
    final github = Uri.parse(issuesUrl).replace(
      query: _query({
        'title': 'Погана відповідь помічника',
        'body': report,
        'labels': 'ai-report',
      }),
    );
    return SafeArea(
      child: SingleChildScrollView(
        padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text('Поскаржитися на відповідь', style: t.titleLarge),
            const SizedBox(height: 8),
            const Text(
              'Якщо відповідь небезпечна, хибна чи образлива, напишіть нам. '
              'Ми перевіримо її і виправимо картки чи налаштування помічника. '
              'Застосунок сам нічого не надсилає: ви побачите лист і вирішите, чи відправляти.',
            ),
            const SizedBox(height: 12),
            Container(
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: Theme.of(context).colorScheme.surfaceContainerHighest,
                borderRadius: BorderRadius.circular(12),
              ),
              constraints: const BoxConstraints(maxHeight: 200),
              child: SingleChildScrollView(
                child: Text(report, style: t.bodySmall),
              ),
            ),
            const SizedBox(height: 12),
            FilledButton.icon(
              onPressed: () => _open(context, mail),
              icon: const Icon(Icons.mail_outline),
              label: const Text('Написати листом'),
            ),
            const SizedBox(height: 8),
            OutlinedButton.icon(
              onPressed: () => _open(context, github),
              icon: const Icon(Icons.open_in_new),
              label: const Text('Написати на GitHub'),
            ),
            const SizedBox(height: 8),
            TextButton.icon(
              onPressed: () async {
                await Clipboard.setData(ClipboardData(text: report));
                if (!context.mounted) return;
                ScaffoldMessenger.of(context).showSnackBar(
                  const SnackBar(content: Text('Текст скарги скопійовано')),
                );
                Navigator.of(context).pop();
              },
              icon: const Icon(Icons.copy),
              label: const Text('Скопіювати текст'),
            ),
          ],
        ),
      ),
    );
  }
}

/// `Uri(queryParameters:)` кодує пробіли як «+», а поштові клієнти
/// показують їх буквально. Тут пробіли кодуються як %20.
String _query(Map<String, String> params) => params.entries
    .map((e) => '${Uri.encodeComponent(e.key)}=${Uri.encodeComponent(e.value)}')
    .join('&');
