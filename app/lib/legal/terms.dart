import 'dart:io';

import 'package:flutter/services.dart' show rootBundle;
import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/screens/common.dart';
import 'package:uautonomy/widgets/markdown_view.dart';

/// Редакція умов. Коли умови суттєво змінюються, міняйте її тут і в
/// `site/terms.html`: застосунок попросить погодитися ще раз.
const termsVersion = '2026-11-25';

const termsAsset = 'assets/legal/terms_uk.md';
const gemmaTermsAsset = 'assets/legal/GEMMA_TERMS_OF_USE.txt';
const gemmaPolicyAsset = 'assets/legal/GEMMA_PROHIBITED_USE_POLICY.txt';

Future<File> _acceptanceFile() async =>
    File('${await appDataDir()}/terms_accepted');

/// Чи погодився користувач з поточною редакцією умов.
Future<bool> termsAccepted() async {
  try {
    final f = await _acceptanceFile();
    return await f.exists() && (await f.readAsString()).trim() == termsVersion;
  } catch (_) {
    return false;
  }
}

Future<void> acceptTerms() async {
  final f = await _acceptanceFile();
  await f.parent.create(recursive: true);
  await f.writeAsString(termsVersion);
}

/// Перед першим використанням ШІ показує умови і просить погодитися.
/// Повертає true, якщо умови прийнято (зараз або раніше).
Future<bool> ensureTermsAccepted(BuildContext context) async {
  if (await termsAccepted()) return true;
  if (!context.mounted) return false;
  final ok = await showDialog<bool>(
    context: context,
    barrierDismissible: false,
    builder: (ctx) => AlertDialog(
      title: const Text('Умови використання ШІ'),
      content: const SingleChildScrollView(
        child: Text(
          'Відповіді ШІ можуть бути хибними. Помічник не замінює лікаря, '
          'рятувальників чи офіційні сповіщення, а медичні картки ще не '
          'перевірив лікар. $emergencyNote\n\n'
          'Моделі не можна використовувати з порушенням закону чи для '
          'заборонених цілей (Gemma Prohibited Use Policy): злочини, насильство, '
          'ненависть, шахрайство, стеження, обман, видавання себе за фахівця тощо.\n\n'
          'Погоджуючись, ви приймаєте Умови використання «Автономії».',
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(
            ctx,
          ).push(MaterialPageRoute<void>(builder: (_) => const TermsScreen())),
          child: const Text('Прочитати повністю'),
        ),
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(false),
          child: const Text('Скасувати'),
        ),
        FilledButton(
          onPressed: () => Navigator.of(ctx).pop(true),
          child: const Text('Погоджуюсь'),
        ),
      ],
    ),
  );
  if (ok != true) return false;
  await acceptTerms();
  return true;
}

/// Повний текст умов українською і посилання на оригінали умов Gemma.
class TermsScreen extends StatelessWidget {
  const TermsScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Умови використання')),
      body: FutureBuilder<String>(
        future: rootBundle.loadString(termsAsset),
        builder: (context, snap) {
          if (!snap.hasData) {
            return const Center(child: CircularProgressIndicator());
          }
          return ListView(
            padding: const EdgeInsets.all(16),
            children: [
              MarkdownView(data: snap.data!),
              const SizedBox(height: 16),
              ListTile(
                leading: const Icon(Icons.description),
                title: const Text('Gemma Terms of Use (англ.)'),
                onTap: () =>
                    _openText(context, 'Gemma Terms of Use', gemmaTermsAsset),
              ),
              ListTile(
                leading: const Icon(Icons.description),
                title: const Text('Gemma Prohibited Use Policy (англ.)'),
                onTap: () => _openText(
                  context,
                  'Gemma Prohibited Use Policy',
                  gemmaPolicyAsset,
                ),
              ),
            ],
          );
        },
      ),
    );
  }

  void _openText(BuildContext context, String title, String asset) {
    Navigator.of(context).push(
      MaterialPageRoute<void>(
        builder: (_) => Scaffold(
          appBar: AppBar(title: Text(title)),
          body: FutureBuilder<String>(
            future: rootBundle.loadString(asset),
            builder: (context, snap) => snap.hasData
                ? SingleChildScrollView(
                    padding: const EdgeInsets.all(16),
                    child: SelectableText(snap.data!),
                  )
                : const Center(child: CircularProgressIndicator()),
          ),
        ),
      ),
    );
  }
}
