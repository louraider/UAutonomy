import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/ai/ai_service.dart';
import 'package:uautonomy/legal/terms.dart';
import 'package:uautonomy/screens/common.dart';
import 'package:uautonomy/screens/library_screen.dart';
import 'package:uautonomy/src/rust/api/ai.dart';
import 'package:uautonomy/src/rust/api/device.dart';
import 'package:uautonomy/src/rust/api/library.dart';
import 'package:uautonomy/widgets/brand.dart';
import 'package:uautonomy/widgets/markdown_view.dart';

/// Помічник: локальна модель відповідає з опорою на бібліотеку.
class ChatScreen extends StatefulWidget {
  const ChatScreen({
    super.key,
    required this.version,
    required this.onOpenPacks,
  });

  final ValueListenable<int> version;
  final VoidCallback onOpenPacks;

  @override
  State<ChatScreen> createState() => _ChatScreenState();
}

class _ChatScreenState extends State<ChatScreen> {
  final _ai = AiService();
  final _input = TextEditingController();
  final _answers = <AiAnswer>[];
  List<LocalModel> _models = const [];

  /// Вибрана модель. Вибирається сама (рекомендована або єдина), а в
  /// пам'ять завантажується при першому запитанні, щоб не забирати ОЗП,
  /// поки помічником не користуються.
  LocalModel? _selected;
  RecommendationView? _rec;
  bool _loading = false;
  bool _busy = false;
  String? _diag;

  @override
  void initState() {
    super.initState();
    widget.version.addListener(_refresh);
    _refresh();
    recommendation().then((r) {
      if (mounted) {
        setState(() => _rec = r);
        _autoSelect();
      }
    });
  }

  @override
  void dispose() {
    widget.version.removeListener(_refresh);
    _ai.unload();
    _input.dispose();
    super.dispose();
  }

  void _refresh() {
    setState(() => _models = localModels());
    _autoSelect();
    recommendation().then((r) {
      if (mounted) setState(() => _rec = r);
    });
  }

  void _autoSelect() {
    final sel = _selected;
    if (sel != null && _models.any((m) => m.path == sel.path)) return;
    if (_models.isEmpty) {
      setState(() => _selected = null);
      return;
    }
    final rec = _rec?.modelId;
    setState(
      () => _selected =
          _models.where((m) => m.id == rec).firstOrNull ?? _models.first,
    );
  }

  /// Завантажує модель у пам'ять. Перед першим використанням ШІ просить
  /// погодитися з умовами; false, якщо користувач відмовився чи сталася помилка.
  Future<bool> _load(LocalModel m) async {
    if (!await ensureTermsAccepted(context)) return false;
    if (!mounted) return false;
    setState(() => _loading = true);
    try {
      _selected = m;
      final before = await ramStatus();
      await _ai.load(m);
      final after = await ramStatus();
      final used = before.availableMb.toInt() - after.availableMb.toInt();
      final backend = _ai.backendName;
      final layers = _ai.gpuLayers;
      _diag = [
        'Завантажено за ${(_ai.lastLoadTime!.inMilliseconds / 1000).toStringAsFixed(1)} с',
        'зайняло ≈${(used / 1024).toStringAsFixed(1)} ГБ ОЗП',
        'вільно ${(after.availableMb.toInt() / 1024).toStringAsFixed(1)} ГБ',
        if (backend != null)
          layers == null || layers <= 0
              ? 'працює на: $backend (процесор)'
              : 'працює на: $backend, $layers шарів на відеокарті',
        if (_ai.gpuError != null)
          'відеокарта недоступна: ${_ai.gpuError!.length > 160 ? '${_ai.gpuError!.substring(0, 160)}…' : _ai.gpuError}',
      ].join(' · ');
      return true;
    } catch (e) {
      if (mounted) showError(context, 'Не вдалося запустити модель: $e');
      return false;
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _importModel() async {
    final res = await FilePicker.pickFiles(dialogTitle: 'Файл моделі (.gguf)');
    if (res.isEmpty) return;
    setState(() => _loading = true);
    try {
      // Великий файл копіюємо одразу в теку моделей, без проміжної копії.
      final dir = '${await appDataDir()}/models';
      final path = await localPathFor(res.single, dir);
      final m = await importModelFile(path: path);
      _refresh();
      await _load(m);
    } catch (e) {
      if (mounted) showError(context, e);
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _send() async {
    final q = _input.text.trim();
    if (q.isEmpty || _busy) return;
    _input.clear();
    setState(() => _busy = true);
    final sel = _selected;
    if (!_ai.isLoaded && sel != null && askRoute(question: q).mode != 'cards') {
      if (!await _load(sel)) {
        if (mounted) {
          _input.text = q;
          setState(() => _busy = false);
        }
        return;
      }
    }
    _ai.ask(q).listen((a) {
      if (!mounted) return;
      setState(() {
        if (!_answers.contains(a)) _answers.insert(0, a);
        if (a.done) _busy = false;
      });
    });
  }

  Widget _modelBar() {
    final current = _ai.model;
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
      child: Row(
        children: [
          Expanded(
            child: DropdownButton<String>(
              isExpanded: true,
              value: (current ?? _selected)?.path,
              hint: const Text('Оберіть модель'),
              items: [
                for (final m in _models)
                  DropdownMenuItem(
                    value: m.path,
                    child: Text(
                      '${m.title} · ${formatBytes(m.sizeBytes.toInt())}',
                    ),
                  ),
              ],
              onChanged: _loading
                  ? null
                  : (p) => _load(_models.firstWhere((m) => m.path == p)),
            ),
          ),
        ],
      ),
    );
  }

  Widget _noModel() {
    final rec = _rec;
    return ListView(
      padding: const EdgeInsets.all(24),
      children: [
        const Icon(Icons.memory, size: 48),
        const SizedBox(height: 12),
        Text(
          'Помічник працює на пристрої без інтернету, але потрібна модель ШІ.',
          style: Theme.of(context).textTheme.titleMedium,
          textAlign: TextAlign.center,
        ),
        const SizedBox(height: 12),
        if (rec != null) Text(rec.reason, textAlign: TextAlign.center),
        const SizedBox(height: 24),
        FilledButton.icon(
          onPressed: widget.onOpenPacks,
          icon: const Icon(Icons.download),
          label: const Text('Завантажити модель у «Пакетах»'),
        ),
        const SizedBox(height: 8),
        OutlinedButton.icon(
          onPressed: _importModel,
          icon: const Icon(Icons.upload_file),
          label: const Text('Вибрати файл моделі'),
        ),
        const SizedBox(height: 24),
        const Text(
          'Без моделі пошук і картки в «Довідці» працюють повністю.',
          textAlign: TextAlign.center,
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    return ContentWidth(
      child: Column(
        children: [
          TabHeader(
            title: 'Помічник',
            trailing: [
              IconButton(
                tooltip: 'Умови використання',
                icon: const Icon(Icons.gavel),
                onPressed: () => Navigator.of(context).push(
                  MaterialPageRoute<void>(builder: (_) => const TermsScreen()),
                ),
              ),
              IconButton(
                tooltip: 'Додати свою модель з файлу',
                icon: const Icon(Icons.upload_file),
                onPressed: _loading ? null : _importModel,
              ),
            ],
          ),
          Expanded(child: _body()),
        ],
      ),
    );
  }

  Widget _body() {
    if (_models.isEmpty) return _noModel();
    return Column(
      children: [
        _modelBar(),
        const Padding(
          padding: EdgeInsets.symmetric(horizontal: 16, vertical: 4),
          child: Text(
            'Відповіді ШІ можуть містити помилки. Завжди перевіряйте за джерелом. $emergencyNote',
            style: TextStyle(fontSize: 12.5, height: 1.35),
          ),
        ),
        if (_loading) const LinearProgressIndicator(),
        if (_diag != null && !_loading)
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Text(_diag!, style: const TextStyle(fontSize: 12)),
          ),
        Expanded(
          child: ListView.builder(
            reverse: true,
            padding: const EdgeInsets.all(12),
            itemCount: _answers.length,
            itemBuilder: (context, i) => _AnswerTile(answer: _answers[i]),
          ),
        ),
        Padding(
          padding: const EdgeInsets.all(12),
          child: Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _input,
                  enabled: !_loading,
                  minLines: 1,
                  maxLines: 4,
                  textInputAction: TextInputAction.send,
                  onSubmitted: (_) => _send(),
                  decoration: InputDecoration(
                    hintText: _ai.isLoaded || _selected != null
                        ? 'Запитайте, наприклад: як очистити воду?'
                        : 'Медичні картки знайдуться і без моделі',
                    border: const OutlineInputBorder(),
                  ),
                ),
              ),
              const SizedBox(width: 8),
              _busy
                  ? IconButton.filledTonal(
                      tooltip: 'Зупинити',
                      icon: const Icon(Icons.stop),
                      onPressed: () async {
                        await _ai.stop();
                        setState(() => _busy = false);
                      },
                    )
                  : IconButton.filled(
                      tooltip: 'Надіслати',
                      icon: const Icon(Icons.send),
                      onPressed: _loading ? null : _send,
                    ),
            ],
          ),
        ),
      ],
    );
  }
}

class _AnswerTile extends StatelessWidget {
  const _AnswerTile({required this.answer});

  final AiAnswer answer;

  @override
  Widget build(BuildContext context) {
    final t = Theme.of(context).textTheme;
    final scheme = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Align(
            alignment: Alignment.centerRight,
            child: Container(
              padding: const EdgeInsets.all(10),
              decoration: BoxDecoration(
                color: scheme.primaryContainer,
                borderRadius: BorderRadius.circular(12),
              ),
              child: Text(answer.question),
            ),
          ),
          const SizedBox(height: 8),
          if (answer.route != null)
            _RoutedAnswer(route: answer.route!)
          else if (answer.error != null)
            Text(
              'Помилка: ${answer.error}',
              style: TextStyle(color: scheme.error),
            )
          else if (answer.text.isEmpty && !answer.done)
            const Align(
              alignment: Alignment.centerLeft,
              child: CircularProgressIndicator(),
            )
          else
            MarkdownView(data: cleanAiText(answer.text.toString())),
          if (answer.done &&
              answer.error == null &&
              answer.firstTokenMs != null)
            Padding(
              padding: const EdgeInsets.only(top: 4),
              child: Text(
                [
                  'перше слово за ${(answer.firstTokenMs! / 1000).toStringAsFixed(1)} с',
                  if (answer.tokensPerSecond != null)
                    '≈${answer.tokensPerSecond!.toStringAsFixed(1)} токен/с',
                ].join(' · '),
                style: t.bodySmall,
              ),
            ),
          if (answer.sources.isNotEmpty) ...[
            const SizedBox(height: 8),
            Wrap(
              spacing: 6,
              runSpacing: 6,
              children: [
                for (final s in answer.sources)
                  ActionChip(
                    avatar: Text('${s.number}'),
                    label: Text(
                      s.reviewed ? s.title : '${s.title} · не перевірено',
                      overflow: TextOverflow.ellipsis,
                    ),
                    onPressed: () => _openSource(context, s.key),
                  ),
              ],
            ),
          ] else if (answer.done && answer.route == null)
            Text(
              'У бібліотеці не знайдено джерел за цим запитанням.',
              style: t.bodySmall,
            ),
        ],
      ),
    );
  }

  void _openSource(BuildContext context, String key) {
    if (key.startsWith('card:')) {
      final c = card(id: key.substring(5));
      if (c != null) openCard(context, c);
    } else {
      Navigator.of(
        context,
      ).push(MaterialPageRoute<void>(builder: (_) => DocScreen(docKey: key)));
    }
  }
}

/// Відповідь без моделі: повідомлення і картки з дослівним текстом джерела.
class _RoutedAnswer extends StatelessWidget {
  const _RoutedAnswer({required this.route});

  final RouteView route;

  @override
  Widget build(BuildContext context) {
    final t = Theme.of(context).textTheme;
    final scheme = Theme.of(context).colorScheme;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Container(
          padding: const EdgeInsets.all(10),
          decoration: BoxDecoration(
            color: scheme.secondaryContainer,
            borderRadius: BorderRadius.circular(12),
          ),
          child: Text(route.message, style: t.bodyLarge),
        ),
        for (final c in route.cards)
          Card(
            margin: const EdgeInsets.only(top: 8),
            child: ListTile(
              leading: const Icon(Icons.bolt),
              title: Text(c.title),
              subtitle: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  if (c.summary != null) Text(c.summary!),
                  if (!c.reviewed) ...[
                    const SizedBox(height: 4),
                    UnreviewedBadge(who: c.reviewRequired),
                  ],
                ],
              ),
              trailing: const Icon(Icons.chevron_right),
              onTap: () => openCard(context, c),
            ),
          ),
      ],
    );
  }
}
