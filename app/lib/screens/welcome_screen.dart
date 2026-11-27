import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/downloads/download_manager.dart';
import 'package:uautonomy/downloads/download_widgets.dart';
import 'package:uautonomy/screens/common.dart';
import 'package:uautonomy/widgets/brand.dart';

/// Перший запуск: пояснюємо, що треба завантажити, і даємо одну кнопку.
/// Раніше людина бачила порожню карту й не розуміла, звідки її взяти.
class WelcomeScreen extends StatelessWidget {
  const WelcomeScreen({
    super.key,
    required this.manager,
    required this.prefs,
    required this.onDone,
  });

  final DownloadManager manager;
  final DownloadPrefs? prefs;

  /// `started` — людина запустила завантаження (тоді ведемо в «Пакети»).
  final void Function(bool started) onDone;

  @override
  Widget build(BuildContext context) {
    final t = Theme.of(context).textTheme;
    return Scaffold(
      body: SafeArea(
        child: ContentWidth(
          child: ListView(
            padding: const EdgeInsets.all(16),
            children: [
              const TabHeader(title: 'Вітаємо в «Автономії»'),
              Text(
                'Термінові картки (кровотеча, опіки, укриття) вже на телефоні й '
                'працюють без інтернету.',
                style: t.bodyLarge,
              ),
              const SizedBox(height: 8),
              Text(
                'Щоб мати ще й карту, медичну енциклопедію і помічника ШІ, '
                'завантажте їх зараз, поки є зв’язок. Потім усе працюватиме офлайн.',
                style: t.bodyLarge,
              ),
              const SizedBox(height: 16),
              RecommendedSetCard(
                manager: manager,
                prefs: prefs,
                elevated: false,
                onStarted: () => onDone(true),
              ),
              const SizedBox(height: 16),
              TextButton(
                onPressed: () => onDone(false),
                child: const Text('Пізніше (усе є на вкладці «Пакети»)'),
              ),
              const SizedBox(height: 8),
              Text(
                emergencyNote,
                style: t.bodySmall,
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      ),
    );
  }
}
