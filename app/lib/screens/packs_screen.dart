import 'dart:async';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:material_ui/material_ui.dart';
import 'package:path_provider/path_provider.dart';
import 'package:uautonomy/screens/common.dart';
import 'package:uautonomy/src/rust/api/device.dart';
import 'package:uautonomy/src/rust/api/packs.dart';

const _tierTitles = {
  'base': 'Базовий',
  'pocket': 'Кишеньковий',
  'family': 'Сімейний',
  'community': 'Громадський',
};

const _kindIcons = {
  'content': Icons.menu_book,
  'map': Icons.map,
  'model': Icons.memory,
};

/// Пакети: що є на пристрої, завантаження з докачкою (інтернет, хаб чи
/// інший телефон) та імпорт з флешки або файлу.
class PacksScreen extends StatefulWidget {
  const PacksScreen({super.key, required this.onChanged});

  final VoidCallback onChanged;

  @override
  State<PacksScreen> createState() => _PacksScreenState();
}

class _PacksScreenState extends State<PacksScreen> {
  final _url = TextEditingController();
  List<PackView> _installed = const [];
  List<PackView> _catalog = const [];
  HardwareView? _hw;
  RecommendationView? _rec;
  String? _busyId;
  double? _progress;
  bool _fetching = false;

  @override
  void initState() {
    super.initState();
    _refresh();
    hardwareInfo().then((h) {
      if (mounted) setState(() => _hw = h);
    });
    recommendation().then((r) {
      if (mounted) setState(() => _rec = r);
    });
  }

  @override
  void dispose() {
    _url.dispose();
    super.dispose();
  }

  void _refresh() => setState(() => _installed = installedPacks());

  void _changed() {
    _refresh();
    widget.onChanged();
  }

  Future<void> _fetch() async {
    if (_url.text.trim().isEmpty) return;
    setState(() => _fetching = true);
    try {
      final c = await fetchCatalog(url: _url.text);
      setState(() => _catalog = c);
    } catch (e) {
      if (mounted) showError(context, 'Не вдалося відкрити каталог: $e');
    } finally {
      if (mounted) setState(() => _fetching = false);
    }
  }

  Future<void> _run(
    String id,
    Stream<ProgressView> stream,
    String okText,
  ) async {
    setState(() {
      _busyId = id;
      _progress = null;
    });
    try {
      await for (final p in stream) {
        if (!mounted) return;
        setState(
          () => _progress = p.total == BigInt.zero ? null : p.done / p.total,
        );
      }
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text(okText)));
      }
      _catalog = [
        for (final p in _catalog)
          p.id == id
              ? PackView(
                  id: p.id,
                  title: p.title,
                  description: p.description,
                  tier: p.tier,
                  kind: p.kind,
                  version: p.version,
                  sizeBytes: p.sizeBytes,
                  installed: true,
                  region: p.region,
                  modelId: p.modelId,
                )
              : p,
      ];
      _changed();
    } catch (e) {
      if (mounted) showError(context, e);
    } finally {
      if (mounted) {
        setState(() {
          _busyId = null;
          _progress = null;
        });
      }
    }
  }

  Future<void> _importFile() async {
    final res = await FilePicker.pickFiles(
      dialogTitle: 'Файл пакета (.uapack)',
    );
    if (res.isEmpty) return;
    final tmp = '${await appDataDir()}/staging/picked';
    final path = await localPathFor(res.single, tmp);
    await _run('import', importPack(path: path), 'Пакет імпортовано');
    if (path.startsWith(tmp)) {
      try {
        await File(path).delete();
      } catch (_) {}
    }
  }

  Future<void> _importFolder() async {
    // ignore: deprecated_member_use
    final dir = await FilePicker.getDirectoryPath(
      dialogTitle: 'Тека пакета з manifest.json',
    );
    if (dir == null) return;
    await _run('import', importPack(path: dir), 'Пакет імпортовано');
  }

  Future<void> _export(PackView p) async {
    // На Android пишемо в теку застосунку на картці пам'яті (видно через
    // USB-кабель у Android/data/org.uautonomy.app/files); на ПК питаємо теку.
    String? dir;
    if (Platform.isAndroid) {
      dir = (await getExternalStorageDirectory())?.path;
    } else {
      // ignore: deprecated_member_use
      dir = await FilePicker.getDirectoryPath(
        dialogTitle: 'Куди зберегти пакет',
      );
    }
    if (dir == null) return;
    final out = '$dir/${p.id}.uapack';
    try {
      await exportPack(id: p.id, outPath: out);
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text('Збережено: $out')));
      }
    } catch (e) {
      if (mounted) showError(context, e);
    }
  }

  Future<void> _remove(PackView p) async {
    final ok = await showDialog<bool>(
      context: context,
      builder: (c) => AlertDialog(
        title: Text('Видалити «${p.title}»?'),
        content: Text(
          'Звільниться ${formatBytes(p.sizeBytes.toInt())}. Пакет можна буде завантажити знову.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(c, false),
            child: const Text('Скасувати'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(c, true),
            child: const Text('Видалити'),
          ),
        ],
      ),
    );
    if (ok != true) return;
    try {
      await removePack(id: p.id);
      _changed();
    } catch (e) {
      if (mounted) showError(context, e);
    }
  }

  Widget _deviceCard() {
    final h = _hw;
    final r = _rec;
    return Card(
      margin: const EdgeInsets.all(16),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Ваш пристрій',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 8),
            if (h == null)
              const LinearProgressIndicator()
            else
              Text(
                'ОЗП ${(h.totalRamMb.toInt() / 1024).toStringAsFixed(1)} ГБ · '
                '${h.cpuCores} ядер · вільно ${(h.freeDiskMb.toInt() / 1024).toStringAsFixed(1)} ГБ',
              ),
            if (r != null) ...[
              const SizedBox(height: 8),
              Text(r.reason),
              Text(
                'Рекомендований набір: ${_tierTitles[r.packTier] ?? r.packTier}',
              ),
            ],
          ],
        ),
      ),
    );
  }

  Widget _packTile(PackView p, {required bool fromCatalog}) {
    final busy = _busyId == p.id;
    final recommendedModel = _rec?.modelId;
    final isRecommended =
        p.kind == 'model' && p.modelId != null && p.modelId == recommendedModel;
    return ListTile(
      leading: Icon(_kindIcons[p.kind] ?? Icons.inventory_2),
      title: Text(p.title),
      subtitle: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            [
              _tierTitles[p.tier] ?? p.tier,
              formatBytes(p.sizeBytes.toInt()),
              'v${p.version}',
              if (isRecommended) 'рекомендовано',
            ].join(' · '),
          ),
          if (p.description.isNotEmpty) Text(p.description),
          if (busy) LinearProgressIndicator(value: _progress),
        ],
      ),
      trailing: fromCatalog
          ? (p.installed
                ? const Icon(Icons.check_circle)
                : busy
                ? IconButton(
                    tooltip: 'Зупинити (продовжиться з місця зупинки)',
                    icon: const Icon(Icons.pause),
                    onPressed: cancelDownload,
                  )
                : IconButton(
                    tooltip: 'Завантажити',
                    icon: const Icon(Icons.download),
                    onPressed: _busyId != null
                        ? null
                        : () => _run(
                            p.id,
                            downloadPack(id: p.id),
                            'Встановлено: ${p.title}',
                          ),
                  ))
          : PopupMenuButton<String>(
              onSelected: (v) => v == 'export' ? _export(p) : _remove(p),
              itemBuilder: (_) => const [
                PopupMenuItem(
                  value: 'export',
                  child: Text('Зберегти у файл (.uapack)'),
                ),
                PopupMenuItem(value: 'remove', child: Text('Видалити')),
              ],
            ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final t = Theme.of(context).textTheme;
    return ListView(
      children: [
        _deviceCard(),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16),
          child: Text('На пристрої', style: t.titleMedium),
        ),
        if (_installed.isEmpty)
          const ListTile(title: Text('Лише базовий пакет (термінові картки)')),
        for (final p in _installed) _packTile(p, fromCatalog: false),
        if (_busyId == 'import') const LinearProgressIndicator(),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          child: Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              OutlinedButton.icon(
                onPressed: _busyId != null ? null : _importFile,
                icon: const Icon(Icons.usb),
                label: const Text('З флешки чи файлу'),
              ),
              if (!Platform.isAndroid)
                if (!Platform.isAndroid)
                  OutlinedButton.icon(
                    onPressed: _busyId != null ? null : _importFolder,
                    icon: const Icon(Icons.folder_open),
                    label: const Text('З теки'),
                  ),
            ],
          ),
        ),
        const Divider(),
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
          child: Text('Завантажити', style: t.titleMedium),
        ),
        Padding(
          padding: const EdgeInsets.all(16),
          child: Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _url,
                  keyboardType: TextInputType.url,
                  decoration: const InputDecoration(
                    labelText: 'Адреса каталогу або іншого пристрою',
                    hintText: 'http://192.168.43.1:8642',
                    border: OutlineInputBorder(),
                  ),
                  onSubmitted: (_) => _fetch(),
                ),
              ),
              const SizedBox(width: 8),
              _fetching
                  ? const CircularProgressIndicator()
                  : IconButton.filled(
                      tooltip: 'Відкрити каталог',
                      icon: const Icon(Icons.arrow_forward),
                      onPressed: _fetch,
                    ),
            ],
          ),
        ),
        const Padding(
          padding: EdgeInsets.symmetric(horizontal: 16),
          child: Text(
            'Адресу іншого телефона чи ПК з «Автономією» видно на його вкладці «Обмін» (там є QR-код). '
            'Перерване завантаження продовжиться з місця зупинки.',
          ),
        ),
        for (final p in _catalog) _packTile(p, fromCatalog: true),
        const SizedBox(height: 24),
      ],
    );
  }
}
