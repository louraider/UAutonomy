import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/ai/ai_service.dart';
import 'package:uautonomy/screens/common.dart';
import 'package:uautonomy/screens/library_screen.dart';
import 'package:uautonomy/src/rust/api/device.dart';
import 'package:uautonomy/src/rust/api/library.dart';

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
  RecommendationView? _rec;
  bool _loading = false;
  bool _busy = false;

  @override
  void initState() {
    super.initState();
    widget.version.addListener(_refresh);
    _refresh();
    recommendation().then((r) {
      if (mounted) setState(() => _rec = r);
    });
  }

  @override
  void dispose() {
    widget.version.removeListener(_refresh);
    _ai.unload();
    _input.dispose();
    super.dispose();
  }

  void _refresh() => setState(() => _models = localModels());

  Future<void> _load(LocalModel m) async {
    setState(() => _loading = true);
    try {
      await _ai.load(m);
    } catch (e) {
      if (mounted) showError(context, 'Не вдалося запустити модель: $e');
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _importModel() async {
    final res = await FilePicker.pickFiles(
      dialogTitle: 'Файл моделі (.gguf або .litertlm)',
    );
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

  void _send() {
    final q = _input.text.trim();
    if (q.isEmpty || _busy) return;
    _input.clear();
    setState(() => _busy = true);
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
              value: current?.path,
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
          IconButton(
            tooltip: 'Додати свою модель з файлу',
            icon: const Icon(Icons.upload_file),
            onPressed: _loading ? null : _importModel,
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
    if (_models.isEmpty) return _noModel();
    return Column(
      children: [
        _modelBar(),
        const Padding(
          padding: EdgeInsets.symmetric(horizontal: 16, vertical: 4),
          child: Text(
            'Відповіді ШІ можуть містити помилки. Завжди перевіряйте за джерелом. $emergencyNote',
            style: TextStyle(fontSize: 12),
          ),
        ),
        if (_loading) const LinearProgressIndicator(),
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
                  enabled: _ai.isLoaded && !_loading,
                  minLines: 1,
                  maxLines: 4,
                  textInputAction: TextInputAction.send,
                  onSubmitted: (_) => _send(),
                  decoration: InputDecoration(
                    hintText: _ai.isLoaded
                        ? 'Запитайте, наприклад: як очистити воду?'
                        : 'Спершу оберіть модель',
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
                      onPressed: _ai.isLoaded ? _send : null,
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
          if (answer.error != null)
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
            SelectableText(answer.text.toString(), style: t.bodyLarge),
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
          ] else if (answer.done)
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
