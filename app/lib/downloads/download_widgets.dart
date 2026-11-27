import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/downloads/download_manager.dart';
import 'package:uautonomy/screens/common.dart';
import 'package:uautonomy/src/rust/api/packs.dart' show PackView, ProgressView;

const kindIcons = {
  'content': Icons.menu_book,
  'map': Icons.map,
  'model': Icons.memory,
};

String progressText(ProgressView? p) {
  if (p == null) return 'Підготовка…';
  final done = formatBytes(p.done.toInt());
  if (p.total == BigInt.zero) return 'Завантажено $done';
  final pct = (p.done / p.total * 100).clamp(0, 100).toStringAsFixed(0);
  return '$pct% · $done з ${formatBytes(p.total.toInt())}';
}

double? progressFraction(ProgressView? p) {
  if (p == null || p.total == BigInt.zero) return null;
  return (p.done / p.total).clamp(0.0, 1.0);
}

/// Рядок стану під назвою пакета: прогрес, «у черзі», помилка.
class PackStatusLine extends StatelessWidget {
  const PackStatusLine({super.key, required this.manager, required this.id});

  final DownloadManager manager;
  final String id;

  @override
  Widget build(BuildContext context) {
    final small = Theme.of(context).textTheme.bodySmall;
    final scheme = Theme.of(context).colorScheme;
    switch (manager.stateOf(id)) {
      case PackState.downloading:
        return Padding(
          padding: const EdgeInsets.only(top: 8),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              LinearProgressIndicator(
                value: progressFraction(manager.progress),
              ),
              const SizedBox(height: 4),
              Text(progressText(manager.progress), style: small),
            ],
          ),
        );
      case PackState.paused:
        final p = manager.current == id ? manager.progress : null;
        return Padding(
          padding: const EdgeInsets.only(top: 8),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              if (p != null)
                LinearProgressIndicator(value: progressFraction(p)),
              const SizedBox(height: 4),
              Text(
                p == null ? 'Призупинено' : 'Призупинено · ${progressText(p)}',
                style: small,
              ),
            ],
          ),
        );
      case PackState.queued:
        return Padding(
          padding: const EdgeInsets.only(top: 4),
          child: Text('У черзі', style: small),
        );
      case PackState.failed:
        return Padding(
          padding: const EdgeInsets.only(top: 4),
          child: Text(
            manager.error ?? 'Не вдалося завантажити',
            style: small?.copyWith(color: scheme.error),
          ),
        );
      case PackState.installed:
      case PackState.available:
      case PackState.update:
        return const SizedBox.shrink();
    }
  }
}

/// Кнопка праворуч від пакета: завантажити, пауза, продовжити, готово.
class PackActionIcon extends StatelessWidget {
  const PackActionIcon({super.key, required this.manager, required this.pack});

  final DownloadManager manager;
  final PackView pack;

  @override
  Widget build(BuildContext context) {
    final id = pack.id;
    return switch (manager.stateOf(id)) {
      PackState.installed => const Tooltip(
        message: 'Встановлено',
        child: Padding(
          padding: EdgeInsets.all(8),
          child: Icon(Icons.check_circle),
        ),
      ),
      PackState.downloading => IconButton(
        tooltip: 'Призупинити (продовжиться з місця зупинки)',
        icon: const Icon(Icons.pause),
        onPressed: manager.pause,
      ),
      PackState.paused || PackState.failed => IconButton(
        tooltip: 'Продовжити',
        icon: const Icon(Icons.play_arrow),
        onPressed: () => manager.enqueue([id]),
      ),
      PackState.queued => IconButton(
        tooltip: 'Прибрати з черги',
        icon: const Icon(Icons.close),
        onPressed: () => manager.remove(id),
      ),
      PackState.update => IconButton(
        tooltip: 'Оновити · ${formatBytes(pack.sizeBytes.toInt())}',
        icon: const Icon(Icons.update),
        onPressed: () => manager.enqueue([id]),
      ),
      PackState.available => IconButton(
        tooltip: 'Завантажити · ${formatBytes(pack.sizeBytes.toInt())}',
        icon: const Icon(Icons.download),
        onPressed: () => manager.enqueue([id]),
      ),
    };
  }
}

/// Велика кнопка завантаження одного пакета (на «Картах» і в «Помічнику»).
/// Сама відкриває каталог, показує розмір, прогрес і докачування.
class PackDownloadPanel extends StatefulWidget {
  const PackDownloadPanel({
    super.key,
    required this.manager,
    required this.find,
    required this.label,
  });

  final DownloadManager manager;

  /// Як знайти пакет у каталозі.
  final PackView? Function(DownloadManager m) find;

  /// Що завантажуємо, наприклад «карту «Київська»».
  final String label;

  @override
  State<PackDownloadPanel> createState() => _PackDownloadPanelState();
}

class _PackDownloadPanelState extends State<PackDownloadPanel> {
  @override
  void initState() {
    super.initState();
    widget.manager.ensureCatalog();
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: widget.manager,
      builder: (context, _) {
        final m = widget.manager;
        final p = widget.find(m);
        if (p == null) {
          return switch (m.catalogStatus) {
            CatalogStatus.loading || CatalogStatus.none => const Padding(
              padding: EdgeInsets.all(8),
              child: Center(child: CircularProgressIndicator()),
            ),
            CatalogStatus.failed => CatalogProblem(manager: m),
            CatalogStatus.ready => Text(
              'У каталозі поки немає: ${widget.label}.',
              textAlign: TextAlign.center,
            ),
          };
        }
        final state = m.stateOf(p.id);
        final size = formatBytes(p.sizeBytes.toInt());
        final Widget button = switch (state) {
          PackState.available || PackState.update => FilledButton.icon(
            onPressed: () => m.enqueue([p.id]),
            icon: const Icon(Icons.download),
            label: Text('Завантажити ${widget.label} · $size'),
          ),
          PackState.downloading => OutlinedButton.icon(
            onPressed: m.pause,
            icon: const Icon(Icons.pause),
            label: const Text('Призупинити'),
          ),
          PackState.paused || PackState.failed => FilledButton.icon(
            onPressed: () => m.enqueue([p.id]),
            icon: const Icon(Icons.play_arrow),
            label: const Text('Продовжити завантаження'),
          ),
          PackState.queued => OutlinedButton.icon(
            onPressed: () => m.remove(p.id),
            icon: const Icon(Icons.close),
            label: const Text('У черзі · прибрати'),
          ),
          PackState.installed => const Text('Встановлено'),
        };
        return Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            button,
            PackStatusLine(manager: m, id: p.id),
            if (state == PackState.downloading)
              const Padding(
                padding: EdgeInsets.only(top: 8),
                child: Text(
                  'Можна перейти на інші вкладки, завантаження триває. '
                  'Якщо зв’язок пропаде, воно продовжиться з місця зупинки.',
                ),
              ),
          ],
        );
      },
    );
  }
}

/// Каталог не відкрився: найчастіше немає інтернету.
class CatalogProblem extends StatelessWidget {
  const CatalogProblem({super.key, required this.manager});

  final DownloadManager manager;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          manager.catalogError ?? offlineError,
          style: TextStyle(color: scheme.error),
        ),
        const SizedBox(height: 8),
        const Text(
          'Інтернет потрібен лише раз, щоб завантажити файли. Без нього їх можна '
          'взяти з іншого телефона чи ПК з «Автономією» (вкладка «Обмін») або з флешки '
          '(«Пакети» → «З флешки чи файлу»).',
        ),
        const SizedBox(height: 8),
        Align(
          alignment: Alignment.centerLeft,
          child: OutlinedButton.icon(
            onPressed: () => manager.loadCatalog(),
            icon: const Icon(Icons.refresh),
            label: const Text('Спробувати ще раз'),
          ),
        ),
      ],
    );
  }
}

/// Поточне завантаження і черга: що качається, скільки лишилося, пауза.
class DownloadQueueCard extends StatelessWidget {
  const DownloadQueueCard({super.key, required this.manager});

  final DownloadManager manager;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: manager,
      builder: (context, _) {
        final m = manager;
        if (!m.hasWork) return const SizedBox.shrink();
        final t = Theme.of(context).textTheme;
        final scheme = Theme.of(context).colorScheme;
        final current = m.pack(m.queue.first);
        final rest = m.queue.length - 1;
        final paused = m.paused;
        return Card(
          margin: const EdgeInsets.fromLTRB(16, 8, 16, 8),
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Icon(paused ? Icons.pause_circle : Icons.downloading),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        paused ? 'Завантаження призупинено' : 'Завантаження',
                        style: t.titleMedium,
                      ),
                    ),
                    paused
                        ? FilledButton.icon(
                            onPressed: m.resume,
                            icon: const Icon(Icons.play_arrow),
                            label: const Text('Продовжити'),
                          )
                        : OutlinedButton.icon(
                            onPressed: m.pause,
                            icon: const Icon(Icons.pause),
                            label: const Text('Пауза'),
                          ),
                  ],
                ),
                const SizedBox(height: 8),
                Text(current?.title ?? m.queue.first),
                const SizedBox(height: 6),
                LinearProgressIndicator(value: progressFraction(m.progress)),
                const SizedBox(height: 4),
                Text(progressText(m.progress), style: t.bodySmall),
                if (rest > 0) ...[
                  const SizedBox(height: 4),
                  Text(
                    'Далі в черзі: $rest · лишилося ${formatBytes(m.queuedBytes)}',
                    style: t.bodySmall,
                  ),
                ],
                if (m.error != null) ...[
                  const SizedBox(height: 8),
                  Text(m.error!, style: TextStyle(color: scheme.error)),
                  const Text(
                    'Уже завантажене не пропаде: натисніть «Продовжити», коли з’явиться зв’язок.',
                  ),
                ],
              ],
            ),
          ),
        );
      },
    );
  }
}

/// «Усе потрібне одним натиском»: карта своєї області, медична
/// енциклопедія і модель ШІ, з розміром і галочками.
class RecommendedSetCard extends StatefulWidget {
  const RecommendedSetCard({
    super.key,
    required this.manager,
    required this.prefs,
    this.onStarted,
    this.elevated = true,
  });

  final DownloadManager manager;
  final DownloadPrefs? prefs;
  final VoidCallback? onStarted;
  final bool elevated;

  @override
  State<RecommendedSetCard> createState() => _RecommendedSetCardState();
}

class _RecommendedSetCardState extends State<RecommendedSetCard> {
  String? _region;
  final Set<String> _skipped = {};

  @override
  void initState() {
    super.initState();
    _region = widget.prefs?.region;
    widget.manager.ensureCatalog();
  }

  void _setRegion(String? r) {
    setState(() => _region = r);
    widget.prefs?.region = r;
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: widget.manager,
      builder: (context, _) {
        final body = _body(context);
        return widget.elevated
            ? Card(
                margin: const EdgeInsets.fromLTRB(16, 8, 16, 8),
                child: Padding(padding: const EdgeInsets.all(16), child: body),
              )
            : body;
      },
    );
  }

  Widget _body(BuildContext context) {
    final m = widget.manager;
    final t = Theme.of(context).textTheme;
    final header = Text(
      'Усе потрібне для роботи без інтернету',
      style: t.titleMedium,
    );
    if (m.catalogStatus == CatalogStatus.failed) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          header,
          const SizedBox(height: 8),
          CatalogProblem(manager: m),
        ],
      );
    }
    if (m.catalogStatus != CatalogStatus.ready) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          header,
          const SizedBox(height: 12),
          const LinearProgressIndicator(),
          const SizedBox(height: 8),
          const Text('Відкриваємо каталог…'),
        ],
      );
    }
    final regions = m.mapRegions;
    final region = regions.any((r) => r.$1 == _region) ? _region : null;
    final set = m.recommendedSet(regionId: region);
    final chosen = [
      for (final p in set)
        if (!_skipped.contains(p.id)) p,
    ];
    final todo = [
      for (final p in chosen)
        if (m.stateOf(p.id) != PackState.installed) p,
    ];
    var total = 0;
    for (final p in todo) {
      total += p.sizeBytes.toInt();
    }
    final free = m.freeDisk;
    final tooBig = free != null && free > 0 && total + 200e6 > free;
    final allDone = set.isNotEmpty && todo.isEmpty && region != null;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        header,
        const SizedBox(height: 4),
        const Text(
          'Виберіть свою область і натисніть одну кнопку. '
          'Файли завантажаться по черзі, перерване продовжиться з місця зупинки.',
        ),
        const SizedBox(height: 12),
        DropdownButtonFormField<String>(
          initialValue: region,
          isExpanded: true,
          decoration: const InputDecoration(
            labelText: 'Ваша область (для карти)',
            border: OutlineInputBorder(),
          ),
          items: [
            for (final r in regions)
              DropdownMenuItem(value: r.$1, child: Text(r.$2)),
          ],
          onChanged: _setRegion,
        ),
        const SizedBox(height: 8),
        for (final p in set)
          CheckboxListTile(
            contentPadding: EdgeInsets.zero,
            value:
                m.stateOf(p.id) == PackState.installed ||
                !_skipped.contains(p.id),
            onChanged: m.stateOf(p.id) == PackState.installed
                ? null
                : (v) => setState(
                    () =>
                        v == true ? _skipped.remove(p.id) : _skipped.add(p.id),
                  ),
            secondary: Icon(kindIcons[p.kind] ?? Icons.inventory_2),
            title: Text(_label(p)),
            subtitle: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  m.stateOf(p.id) == PackState.installed
                      ? 'Встановлено'
                      : formatBytes(p.sizeBytes.toInt()),
                ),
                PackStatusLine(manager: m, id: p.id),
              ],
            ),
          ),
        const SizedBox(height: 8),
        if (allDone)
          const ListTile(
            contentPadding: EdgeInsets.zero,
            leading: Icon(Icons.check_circle),
            title: Text('Усе рекомендоване вже на пристрої.'),
          )
        else ...[
          if (region == null)
            const Padding(
              padding: EdgeInsets.only(bottom: 8),
              child: Text('Спершу виберіть область, щоб додати її карту.'),
            ),
          if (tooBig)
            Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: Text(
                'Замало місця: потрібно ${formatBytes(total)}, вільно ${formatBytes(free)}. '
                'Зніміть галочку з чогось великого (наприклад, моделі ШІ).',
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ),
          FilledButton.icon(
            onPressed: todo.isEmpty || tooBig
                ? null
                : () {
                    m.enqueue([for (final p in todo) p.id]);
                    widget.onStarted?.call();
                  },
            icon: const Icon(Icons.download),
            label: Text(
              todo.isEmpty
                  ? 'Завантажити все'
                  : 'Завантажити все · ${formatBytes(total)}',
            ),
          ),
          if (total > 1e9)
            Padding(
              padding: const EdgeInsets.only(top: 6),
              child: Text(
                'Великі файли краще качати через Wi‑Fi. Застосунок не закривайте, '
                'доки йде завантаження.',
                style: t.bodySmall,
              ),
            ),
        ],
      ],
    );
  }

  String _label(PackView p) => switch (p.kind) {
    'model' => 'Помічник ШІ: ${p.title}',
    _ => p.title,
  };
}
