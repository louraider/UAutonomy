export 'package:uautonomy/downloads/download_manager.dart'
    show officialCatalogUrl;

import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:material_ui/material_ui.dart';
import 'package:path_provider/path_provider.dart';
import 'package:uautonomy/downloads/download_manager.dart';
import 'package:uautonomy/downloads/download_widgets.dart';
import 'package:uautonomy/legal/terms.dart';
import 'package:uautonomy/screens/common.dart';
import 'package:uautonomy/src/rust/api/device.dart';
import 'package:uautonomy/src/rust/api/packs.dart';
import 'package:uautonomy/widgets/brand.dart';

const _tierTitles = {
  'base': 'Базовий',
  'pocket': 'Кишеньковий',
  'family': 'Сімейний',
  'community': 'Громадський',
};

/// Пакети: що є на пристрої, «усе потрібне одним натиском», каталог за
/// розділами з кнопкою завантаження в кожного пакета, інші джерела
/// (телефон, ПК, хаб) та імпорт з флешки або файлу.
class PacksScreen extends StatefulWidget {
  const PacksScreen({
    super.key,
    required this.onChanged,
    this.manager,
    this.prefs,
  });

  final VoidCallback onChanged;
  final DownloadManager? manager;
  final DownloadPrefs? prefs;

  @override
  State<PacksScreen> createState() => _PacksScreenState();
}

class _PacksScreenState extends State<PacksScreen> {
  late final DownloadManager _m = widget.manager ?? DownloadManager.instance;
  final _url = TextEditingController();
  List<PackView> _installed = const [];
  HardwareView? _hw;
  RecommendationView? _rec;
  String? _busyId;
  ProgressView? _progress;

  @override
  void initState() {
    super.initState();
    _refresh();
    _m.addListener(_onManager);
    _m.ensureCatalog();
    _refreshRecommendation();
  }

  @override
  void dispose() {
    _m.removeListener(_onManager);
    _url.dispose();
    super.dispose();
  }

  int _installedCount = -1;

  /// Після кожного встановленого пакета оновлюємо список «На пристрої».
  void _onManager() {
    if (!mounted) return;
    final n = installedPacks().length;
    if (n != _installedCount) {
      _installedCount = n;
      _installed = installedPacks();
      _refreshRecommendation();
    }
    setState(() {});
  }

  void _refresh() => setState(() {
    _installed = installedPacks();
    _installedCount = _installed.length;
  });

  /// Рекомендація залежить від вільного місця і від того, які моделі є в
  /// каталозі, тож оновлюємо її після відкриття каталогу й змін пакетів.
  void _refreshRecommendation() {
    recommendation().then((r) {
      if (mounted) setState(() => _rec = r);
    });
    hardwareInfo().then((h) {
      if (mounted) setState(() => _hw = h);
    });
  }

  void _changed() {
    _refresh();
    _m.refreshInstalled();
    _refreshRecommendation();
    widget.onChanged();
  }

  Future<void> _openOther() async {
    final u = _url.text.trim();
    if (u.isEmpty) return;
    await _m.loadCatalog(u);
    if (!mounted) return;
    if (_m.catalogStatus == CatalogStatus.failed) {
      showError(context, 'Не вдалося відкрити каталог. ${_m.catalogError}');
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
        setState(() => _progress = p);
      }
      if (mounted) {
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text(okText)));
      }
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
    // iOS дає копію вибраного файлу в тимчасовій теці застосунку: пакет
    // уже імпортовано, тож копію (сотні мегабайтів) прибираємо одразу.
    if (Platform.isIOS) await clearPickedCopies();
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
    // USB-кабель у Android/data/org.uautonomy.app/files); на iPhone у
    // Документи застосунку (видно у «Файли» → «На iPhone» → «Автономія»
    // і через Finder на Mac); на ПК питаємо теку.
    String? dir;
    if (Platform.isAndroid) {
      dir = (await getExternalStorageDirectory())?.path;
    } else if (Platform.isIOS) {
      dir = (await getApplicationDocumentsDirectory()).path;
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

  Widget _catalogTile(PackView p) {
    final isRecommended =
        p.kind == 'model' &&
        p.modelId != null &&
        p.modelId == (_rec?.modelId ?? _m.recommendedModelId);
    return ListTile(
      leading: Icon(kindIcons[p.kind] ?? Icons.inventory_2),
      title: Text(p.title),
      subtitle: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            [
              formatBytes(p.sizeBytes.toInt()),
              if (isRecommended) 'рекомендовано для цього пристрою',
              if (_m.stateOf(p.id) == PackState.update) 'є нова версія',
            ].join(' · '),
          ),
          if (p.description.isNotEmpty) Text(p.description),
          PackStatusLine(manager: _m, id: p.id),
        ],
      ),
      trailing: PackActionIcon(manager: _m, pack: p),
    );
  }

  Widget _installedTile(PackView p) => ListTile(
    leading: Icon(kindIcons[p.kind] ?? Icons.inventory_2),
    title: Text(p.title),
    subtitle: Text(
      [
        _tierTitles[p.tier] ?? p.tier,
        formatBytes(p.sizeBytes.toInt()),
        'v${p.version}',
      ].join(' · '),
    ),
    trailing: PopupMenuButton<String>(
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

  /// Каталог за розділами: помічник ШІ (рекомендована модель першою),
  /// довідники, карти (27 областей згорнуто, щоб не ховали решту).
  List<Widget> _catalogSections(TextTheme t) {
    final rec = _rec?.modelId ?? _m.recommendedModelId;
    final models = _m.catalog.where((p) => p.kind == 'model').toList()
      ..sort((a, b) {
        int r(PackView p) => p.modelId == rec ? 0 : 1;
        return r(a).compareTo(r(b));
      });
    final content = _m.catalog.where((p) => p.kind == 'content').toList();
    final maps = _m.catalog.where((p) => p.kind == 'map').toList()
      ..sort(
        (a, b) =>
            DownloadManager.regionTitle(a)
                .compareTo(DownloadManager.regionTitle(b)),
      );
    final other = _m.catalog
        .where((p) => !const {'model', 'content', 'map'}.contains(p.kind))
        .toList();
    Widget heading(String s) => Padding(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 0),
      child: Text(s, style: t.titleSmall),
    );
    final mapsInstalled = maps
        .where((p) => _m.stateOf(p.id) == PackState.installed)
        .length;
    return [
      if (models.isNotEmpty) ...[
        heading('Помічник ШІ'),
        for (final p in models) _catalogTile(p),
      ],
      if (content.isNotEmpty) ...[
        heading('Довідники й енциклопедії'),
        for (final p in content) _catalogTile(p),
      ],
      if (maps.isNotEmpty)
        ExpansionTile(
          leading: const Icon(Icons.map),
          title: Text('Карти областей (${maps.length})'),
          subtitle: Text(
            mapsInstalled == 0
                ? 'Натисніть, щоб вибрати область'
                : 'Встановлено: $mapsInstalled',
          ),
          children: [for (final p in maps) _catalogTile(p)],
        ),
      for (final p in other) _catalogTile(p),
    ];
  }

  @override
  Widget build(BuildContext context) {
    final t = Theme.of(context).textTheme;
    final importing = _busyId == 'import';
    final fraction = progressFraction(_progress);
    return ContentWidth(
      child: ListView(
        children: [
          const TabHeader(title: 'Пакети'),
          DownloadQueueCard(manager: _m),
          if (_m.isOfficial)
            RecommendedSetCard(manager: _m, prefs: widget.prefs),
          _deviceCard(),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Text('На пристрої', style: t.titleMedium),
          ),
          if (_installed.isEmpty)
            const ListTile(
              title: Text('Лише базовий пакет (термінові картки)'),
            ),
          for (final p in _installed) _installedTile(p),
          if (importing) ...[
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: LinearProgressIndicator(value: fraction),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 4, 16, 0),
              child: Text(progressText(_progress)),
            ),
          ],
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
            child: Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                OutlinedButton.icon(
                  onPressed: _busyId != null || _m.busy ? null : _importFile,
                  icon: const Icon(Icons.usb),
                  label: const Text('З флешки чи файлу'),
                ),
                // Теку на телефоні не вибрати: Android і iOS дають доступ
                // лише до окремих файлів.
                if (!Platform.isAndroid && !Platform.isIOS)
                  OutlinedButton.icon(
                    onPressed: _busyId != null || _m.busy
                        ? null
                        : _importFolder,
                    icon: const Icon(Icons.folder_open),
                    label: const Text('З теки'),
                  ),
              ],
            ),
          ),
          const Divider(),
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
            child: Text(
              _m.isOfficial ? 'Усі пакети' : 'Пакети з іншого пристрою',
              style: t.titleMedium,
            ),
          ),
          if (_m.catalogStatus == CatalogStatus.loading)
            const Padding(
              padding: EdgeInsets.all(16),
              child: LinearProgressIndicator(),
            ),
          if (_m.catalogStatus == CatalogStatus.failed)
            Padding(
              padding: const EdgeInsets.all(16),
              child: CatalogProblem(manager: _m),
            ),
          if (_m.catalogStatus == CatalogStatus.ready)
            ListTile(
              leading: const Icon(Icons.verified_user),
              title: Text(
                _m.catalogUrl.toLowerCase().startsWith('https://')
                    ? 'Підпис каталогу перевірено: його підписано ключем проєкту.'
                    : 'Каталог з іншого пристрою. Перед встановленням застосунок перевірить підпис кожного пакета.',
              ),
            ),
          ..._catalogSections(t),
          const Divider(),
          ExpansionTile(
            leading: const Icon(Icons.devices),
            title: const Text('З іншого телефона, ПК чи хаба'),
            subtitle: const Text('Коли інтернету немає, а поруч є «Автономія»'),
            childrenPadding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
            children: [
              const Text(
                'Адресу іншого телефона чи ПК з «Автономією» видно на його вкладці «Обмін» (там є QR-код).',
              ),
              const SizedBox(height: 8),
              Row(
                children: [
                  Expanded(
                    child: TextField(
                      controller: _url,
                      keyboardType: TextInputType.url,
                      decoration: const InputDecoration(
                        labelText: 'Адреса пристрою',
                        hintText: 'http://192.168.43.1:8642',
                        border: OutlineInputBorder(),
                      ),
                      onSubmitted: (_) => _openOther(),
                    ),
                  ),
                  const SizedBox(width: 8),
                  _m.catalogStatus == CatalogStatus.loading
                      ? const CircularProgressIndicator()
                      : IconButton.filled(
                          tooltip: 'Відкрити каталог',
                          icon: const Icon(Icons.arrow_forward),
                          onPressed: _m.busy ? null : _openOther,
                        ),
                ],
              ),
              if (!_m.isOfficial) ...[
                const SizedBox(height: 8),
                Align(
                  alignment: Alignment.centerLeft,
                  child: TextButton(
                    onPressed: _m.busy
                        ? null
                        : () => _m.loadCatalog(officialCatalogUrl),
                    child: const Text('Повернутися до офіційного каталогу'),
                  ),
                ),
              ],
            ],
          ),
          ListTile(
            leading: const Icon(Icons.gavel),
            title: const Text('Умови використання'),
            subtitle: const Text(
              'Обмеження для моделей ШІ, медичне застереження, ліцензії',
            ),
            trailing: const Icon(Icons.chevron_right),
            onTap: () => Navigator.of(context).push(
              MaterialPageRoute<void>(builder: (_) => const TermsScreen()),
            ),
          ),
          const SizedBox(height: 24),
        ],
      ),
    );
  }
}
