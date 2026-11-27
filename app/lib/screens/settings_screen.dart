import 'dart:io';

import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/distribution.dart';
import 'package:uautonomy/legal/terms.dart';
import 'package:uautonomy/stats/stats.dart';
import 'package:url_launcher/url_launcher.dart';

const privacyUrl = 'https://uautonomy.app/privacy';
const playStoreUrl =
    'https://play.google.com/store/apps/details?id=org.uautonomy.app';

String get _device =>
    Platform.isAndroid || Platform.isIOS ? 'телефоні' : 'комп\'ютері';

/// Що саме надсилає статистика. Той самий перелік є на uautonomy.app/privacy:
/// міняючи одне, міняйте й друге.
const statsWhatIsSent = [
  'які вкладки, розділи й картки відкривають;',
  'чи знайшов пошук щось (без тексту пошуку);',
  'як закінчилось запитання до помічника: була відповідь, не знайшлося джерел, помилка (без тексту запитання й відповіді);',
  'які пакети встановили і чи вдалося;',
  'система (Android, Windows, iOS), номер версії й звідки встановлено застосунок.',
];

/// Запит згоди на статистику. Показується раз, при першому запуску збірки,
/// де статистика є. «Так» основна кнопка, але людина завжди обирає сама.
Future<void> askStatsConsent(BuildContext context, {Stats? stats}) async {
  final s = stats ?? Stats.instance;
  if (!s.needsConsent || !context.mounted) return;
  final yes = await showDialog<bool>(
    context: context,
    barrierDismissible: false,
    builder: (ctx) => AlertDialog(
      title: const Text('Допоможете зробити «Автономію» кращою?'),
      content: SingleChildScrollView(
        child: Text(
          'Застосунок може надсилати нам анонімну статистику: які розділи '
          'відкривають, на які запитання помічник не знайшов відповіді, чи '
          'встановились пакети. Так ми бачимо, що дописати й виправити.\n\n'
          'Тексту ваших запитань, імені, контактів і номера пристрою ми не '
          'отримуємо. Без інтернету статистика чекає на $_device і '
          'надсилається пізніше.\n\n'
          'Передумаєте — вимкніть у налаштуваннях: значок шестерні у вкладці «Довідка».',
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(false),
          child: const Text('Ні'),
        ),
        FilledButton(
          autofocus: true,
          onPressed: () => Navigator.of(ctx).pop(true),
          child: const Text('Так'),
        ),
      ],
    ),
  );
  await s.setConsent(yes ?? false);
  if (yes == true) track('app_open', {'dist': distChannel});
}

/// Налаштування: статистика, відгук, умови й приватність.
class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key, this.stats});

  final Stats? stats;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  Stats get _stats => widget.stats ?? Stats.instance;

  @override
  Widget build(BuildContext context) {
    final t = Theme.of(context).textTheme;
    final available = _stats.available;
    return Scaffold(
      appBar: AppBar(title: const Text('Налаштування')),
      body: ListView(
        padding: const EdgeInsets.only(bottom: 24),
        children: [
          SwitchListTile(
            title: const Text('Анонімна статистика'),
            subtitle: Text(
              available
                  ? 'Показує нам, що дописати й виправити. Тексту запитань не надсилає.'
                  : 'У цій збірці статистики немає.',
            ),
            value: _stats.enabled,
            onChanged: available
                ? (v) {
                    // Рішення діє одразу, файл допишеться слідом.
                    _stats.setConsent(v);
                    setState(() {});
                  }
                : null,
          ),
          if (available)
            ExpansionTile(
              title: const Text('Що саме надсилається'),
              childrenPadding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
              expandedCrossAxisAlignment: CrossAxisAlignment.start,
              children: [
                for (final line in statsWhatIsSent)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 4),
                    child: Text('• $line'),
                  ),
                const SizedBox(height: 8),
                Text(
                  'Статистику приймає Aptabase на серверах у ЄС. Він визначає '
                  'за IP-адресою лише країну, а саму адресу не зберігає. '
                  'Номер сеансу випадковий і новий на кожному запуску, тож '
                  'різні дні однієї людини між собою не пов\'язати. Коли ви '
                  'вимикаєте статистику, ще не надіслані події стираються.',
                  style: t.bodySmall,
                ),
              ],
            ),
          const Divider(),
          ListTile(
            leading: const Icon(Icons.rate_review_outlined),
            title: const Text('Надіслати відгук'),
            subtitle: const Text('Що подобається, чого бракує, що не працює'),
            onTap: () => showFeedbackSheet(context),
          ),
          if (distChannel == 'play')
            ListTile(
              leading: const Icon(Icons.star_outline),
              title: const Text('Оцінити в Google Play'),
              onTap: () => _openUrl(context, playStoreUrl),
            ),
          ListTile(
            leading: const Icon(Icons.gavel_outlined),
            title: const Text('Умови використання'),
            onTap: () => Navigator.of(context).push(
              MaterialPageRoute<void>(builder: (_) => const TermsScreen()),
            ),
          ),
          ListTile(
            leading: const Icon(Icons.privacy_tip_outlined),
            title: const Text('Політика приватності'),
            subtitle: const Text('uautonomy.app/privacy'),
            onTap: () => _openUrl(context, privacyUrl),
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 16, 16, 0),
            child: Text(_versionLine(), style: t.bodySmall),
          ),
        ],
      ),
    );
  }
}

String _versionLine() {
  const build = String.fromEnvironment('UAUTONOMY_BUILD');
  final from = switch (distChannel) {
    'play' => 'Google Play',
    'msstore' => 'Microsoft Store',
    _ => 'uautonomy.app',
  };
  return build.isEmpty
      ? 'Збірка для розробки · $from'
      : 'Збірка $build · $from';
}

Future<void> _openUrl(BuildContext context, String url) async {
  var ok = false;
  try {
    ok = await launchUrl(Uri.parse(url), mode: LaunchMode.externalApplication);
  } catch (_) {}
  if (!ok && context.mounted) {
    ScaffoldMessenger.of(context)
        .showSnackBar(SnackBar(content: Text('Відкрийте в браузері: $url')));
  }
}

/// Відгук іде листом чи на GitHub: людина бачить увесь текст і сама його
/// надсилає. Без інтернету поштовий застосунок тримає лист у вихідних.
Future<void> showFeedbackSheet(BuildContext context) {
  track('feedback_open');
  return showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    showDragHandle: true,
    builder: (_) => const FeedbackSheet(),
  );
}

class FeedbackSheet extends StatefulWidget {
  const FeedbackSheet({super.key});

  @override
  State<FeedbackSheet> createState() => _FeedbackSheetState();
}

class _FeedbackSheetState extends State<FeedbackSheet> {
  final _text = TextEditingController();

  @override
  void dispose() {
    _text.dispose();
    super.dispose();
  }

  String get _body =>
      '${_text.text.trim()}\n\n---\n${_versionLine()} · ${Platform.operatingSystem}';

  Future<void> _send(Uri uri) async {
    var ok = false;
    try {
      ok = await launchUrl(uri, mode: LaunchMode.externalApplication);
    } catch (_) {}
    if (!mounted) return;
    if (!ok) {
      await Clipboard.setData(ClipboardData(text: _body));
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text(
            'Не вдалося відкрити. Текст скопійовано: надішліть його на $supportEmail.',
          ),
        ),
      );
    }
    Navigator.of(context).pop();
  }

  @override
  Widget build(BuildContext context) {
    final t = Theme.of(context).textTheme;
    final empty = _text.text.trim().isEmpty;
    final mail = Uri(
      scheme: 'mailto',
      path: supportEmail,
      query: _query({'subject': 'Автономія: відгук', 'body': _body}),
    );
    final github = Uri.parse(issuesUrl).replace(
      query: _query({'title': 'Відгук', 'body': _body, 'labels': 'feedback'}),
    );
    return Padding(
      padding: EdgeInsets.only(bottom: MediaQuery.viewInsetsOf(context).bottom),
      child: SafeArea(
        child: SingleChildScrollView(
          padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text('Надіслати відгук', style: t.titleLarge),
              const SizedBox(height: 8),
              const Text(
                'Що подобається, чого бракує, що не працює? Лист піде з вашої '
                'пошти, і ви побачите його перед надсиланням. Без інтернету він почекає у вихідних.',
              ),
              const SizedBox(height: 12),
              TextField(
                controller: _text,
                minLines: 4,
                maxLines: 10,
                autofocus: true,
                textCapitalization: TextCapitalization.sentences,
                decoration: const InputDecoration(
                  hintText: 'Ваш відгук',
                  border: OutlineInputBorder(),
                ),
                onChanged: (_) => setState(() {}),
              ),
              const SizedBox(height: 4),
              Text(
                'До тексту додамо номер версії і систему, щоб легше знайти помилку.',
                style: t.bodySmall,
              ),
              const SizedBox(height: 12),
              FilledButton.icon(
                onPressed: empty ? null : () => _send(mail),
                icon: const Icon(Icons.mail_outline),
                label: const Text('Написати листом'),
              ),
              const SizedBox(height: 8),
              OutlinedButton.icon(
                onPressed: empty ? null : () => _send(github),
                icon: const Icon(Icons.open_in_new),
                label: const Text('Написати на GitHub (публічно)'),
              ),
              const SizedBox(height: 8),
              TextButton.icon(
                onPressed: empty
                    ? null
                    : () async {
                        await Clipboard.setData(ClipboardData(text: _body));
                        if (!context.mounted) return;
                        ScaffoldMessenger.of(context).showSnackBar(
                          const SnackBar(content: Text('Текст скопійовано')),
                        );
                        Navigator.of(context).pop();
                      },
                icon: const Icon(Icons.copy),
                label: const Text('Скопіювати текст'),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Пробіли як %20, а не «+»: поштові клієнти показують «+» буквально.
String _query(Map<String, String> params) => params.entries
    .map((e) => '${Uri.encodeComponent(e.key)}=${Uri.encodeComponent(e.value)}')
    .join('&');
