import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:uautonomy/screens/common.dart';
import 'package:uautonomy/src/rust/api/device.dart' as device;
import 'package:uautonomy/src/rust/api/packs.dart' as packs;
import 'package:uautonomy/src/rust/api/packs.dart' show PackView, ProgressView;

/// Офіційний каталог: пакети лежать у GitHub Releases проєкту (тег `packs`).
const officialCatalogUrl =
    'https://github.com/louraider/UAutonomy/releases/download/packs/catalog.json';

/// Невелика медична енциклопедія, яку радимо всім.
const recommendedEncyclopediaId = 'zim-wikipedia-uk-medicine';

/// Те, що менеджеру потрібно від ядра. Окремий інтерфейс, щоб чергу можна
/// було перевірити тестом без Rust і без мережі.
abstract class PackBackend {
  Future<List<PackView>> fetchCatalog(String url);
  Stream<ProgressView> download(String id);
  void cancel();
  List<PackView> installed();

  /// Модель, яку радимо для цього пристрою, з урахуванням каталогу.
  Future<String?> recommendedModelId();
  Future<int> freeDiskBytes();
}

class RustPackBackend implements PackBackend {
  const RustPackBackend();

  @override
  Future<List<PackView>> fetchCatalog(String url) =>
      packs.fetchCatalog(url: url);

  @override
  Stream<ProgressView> download(String id) => packs.downloadPack(id: id);

  @override
  void cancel() => packs.cancelDownload();

  @override
  List<PackView> installed() => packs.installedPacks();

  @override
  Future<String?> recommendedModelId() async =>
      (await device.recommendation()).modelId;

  @override
  Future<int> freeDiskBytes() async =>
      (await device.hardwareInfo()).freeDiskMb.toInt() * 1024 * 1024;
}

enum CatalogStatus { none, loading, ready, failed }

/// Стан окремого пакета для кнопки.
enum PackState {
  available,
  update,
  queued,
  downloading,
  paused,
  failed,
  installed,
}

/// Черга завантажень для всього застосунку. Пакети качаються по одному
/// (ядро вміє одне завантаження за раз), перерване продовжується з місця
/// зупинки. Каталог відкривається сам, без введення адреси.
class DownloadManager extends ChangeNotifier {
  DownloadManager(this._backend, {this.onInstalled});

  final PackBackend _backend;

  /// Викликається після кожного встановленого пакета (оновити бібліотеку).
  VoidCallback? onInstalled;

  static DownloadManager? _instance;

  /// Спільний менеджер застосунку (з Rust-ядром).
  static DownloadManager get instance =>
      _instance ??= DownloadManager(const RustPackBackend());

  @visibleForTesting
  static set instance(DownloadManager m) => _instance = m;

  CatalogStatus catalogStatus = CatalogStatus.none;
  String catalogUrl = officialCatalogUrl;
  String? catalogError;
  List<PackView> catalog = const [];
  String? recommendedModelId;
  int? freeDisk;

  final List<String> _queue = [];
  String? current;
  ProgressView? progress;
  bool paused = false;
  String? error;
  String? failedId;
  bool _pauseRequested = false;
  bool _running = false;
  Set<String> _installedIds = {};
  Map<String, String> _installedVersions = {};

  /// Черга, включно з поточним пакетом.
  List<String> get queue => List.unmodifiable(_queue);

  bool get busy => _running;
  bool get hasWork => _queue.isNotEmpty;
  bool get isOfficial => catalogUrl == officialCatalogUrl;

  PackView? pack(String id) {
    for (final p in catalog) {
      if (p.id == id) return p;
    }
    return null;
  }

  /// Пакет карти для області з каталогу.
  PackView? mapFor(String regionId) {
    for (final p in catalog) {
      if (p.kind == 'map' && p.region == regionId) return p;
    }
    return null;
  }

  /// Пакет моделі з каталогу за ідентифікатором моделі.
  PackView? modelPack(String? modelId) {
    if (modelId == null) return null;
    for (final p in catalog) {
      if (p.kind == 'model' && p.modelId == modelId) return p;
    }
    return null;
  }

  void refreshInstalled() {
    final list = _backend.installed();
    _installedIds = {for (final p in list) p.id};
    _installedVersions = {for (final p in list) p.id: p.version};
    notifyListeners();
  }

  bool isInstalled(String id) {
    final p = pack(id);
    final v = _installedVersions[id];
    if (p == null) return v != null;
    return p.installed || v == p.version;
  }

  PackState stateOf(String id) {
    if (isInstalled(id)) return PackState.installed;
    if (failedId == id) return PackState.failed;
    if (current == id) return paused ? PackState.paused : PackState.downloading;
    if (_queue.contains(id)) {
      return paused ? PackState.paused : PackState.queued;
    }
    if (_installedIds.contains(id)) return PackState.update;
    return PackState.available;
  }

  /// Відкриває каталог, якщо його ще немає або минула спроба не вдалася.
  Future<void> ensureCatalog() async {
    if (catalogStatus == CatalogStatus.ready ||
        catalogStatus == CatalogStatus.loading) {
      return;
    }
    await loadCatalog();
  }

  Future<void> loadCatalog([String? url]) async {
    final u = (url ?? catalogUrl).trim();
    if (u.isEmpty) return;
    catalogUrl = u;
    catalogStatus = CatalogStatus.loading;
    catalogError = null;
    notifyListeners();
    try {
      catalog = await _backend.fetchCatalog(u);
      catalogStatus = CatalogStatus.ready;
    } catch (e) {
      catalogStatus = CatalogStatus.failed;
      catalogError = errorText(e);
    }
    _installedIds = {for (final p in _backend.installed()) p.id};
    _installedVersions = {
      for (final p in _backend.installed()) p.id: p.version,
    };
    notifyListeners();
    // Рекомендація залежить від моделей у каталозі й вільного місця.
    try {
      recommendedModelId = await _backend.recommendedModelId();
      freeDisk = await _backend.freeDiskBytes();
    } catch (_) {}
    notifyListeners();
  }

  /// Ставить пакети в чергу (вже встановлені й ті, що в черзі, пропускає)
  /// і запускає завантаження.
  void enqueue(Iterable<String> ids) {
    for (final id in ids) {
      if (isInstalled(id) || _queue.contains(id)) continue;
      _queue.add(id);
    }
    if (failedId != null && _queue.contains(failedId)) {
      failedId = null;
      error = null;
    }
    paused = false;
    notifyListeners();
    _pump();
  }

  /// Зупиняє поточне завантаження; продовжиться з місця зупинки.
  void pause() {
    if (!_running) {
      paused = _queue.isNotEmpty;
      notifyListeners();
      return;
    }
    _pauseRequested = true;
    _backend.cancel();
  }

  void resume() {
    paused = false;
    error = null;
    failedId = null;
    notifyListeners();
    _pump();
  }

  /// Прибирає пакет з черги (поточний зупиняється, частина файлу лишається
  /// для докачування пізніше).
  void remove(String id) {
    if (current == id && _running) {
      _queue.remove(id);
      _pauseRequested = true;
      _backend.cancel();
      // Решта черги піде далі, щойно поточне завантаження зупиниться.
      _resumeAfterStop = true;
    } else {
      _queue.remove(id);
      if (failedId == id) {
        failedId = null;
        error = null;
      }
    }
    notifyListeners();
  }

  bool _resumeAfterStop = false;

  Future<void> _pump() async {
    if (_running || paused) return;
    _running = true;
    try {
      while (_queue.isNotEmpty && !paused) {
        final id = _queue.first;
        current = id;
        progress = null;
        notifyListeners();
        try {
          await for (final p in _backend.download(id)) {
            progress = p;
            notifyListeners();
          }
          _queue.remove(id);
          _installedIds.add(id);
          final v = pack(id)?.version;
          if (v != null) _installedVersions[id] = v;
          catalog = [
            for (final p in catalog) p.id == id ? _withInstalled(p) : p,
          ];
          onInstalled?.call();
        } catch (e) {
          if (_pauseRequested) {
            _pauseRequested = false;
            if (_resumeAfterStop) {
              _resumeAfterStop = false;
            } else {
              paused = true;
            }
          } else {
            failedId = id;
            error = errorText(e);
            paused = true;
          }
        }
      }
    } finally {
      _running = false;
      current = paused && _queue.isNotEmpty ? _queue.first : null;
      progress = paused ? progress : null;
      _pauseRequested = false;
      _resumeAfterStop = false;
      notifyListeners();
      try {
        freeDisk = await _backend.freeDiskBytes();
        notifyListeners();
      } catch (_) {}
    }
  }

  /// Скільки байтів лишилося завантажити в черзі.
  int get queuedBytes {
    var sum = 0;
    for (final id in _queue) {
      sum += pack(id)?.sizeBytes.toInt() ?? 0;
    }
    final p = progress;
    if (p != null) sum -= p.done.toInt();
    return sum < 0 ? 0 : sum;
  }

  static PackView _withInstalled(PackView p) => PackView(
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
  );

  /// Що радимо завантажити: карту своєї області (для Київської ще й
  /// Київ), медичну енциклопедію і модель ШІ, яка потягне на пристрої.
  List<PackView> recommendedSet({String? regionId}) {
    final out = <PackView>[];
    if (regionId != null) {
      final m = mapFor(regionId);
      if (m != null) out.add(m);
      if (regionId == 'kyivska') {
        final k = mapFor('kyiv');
        if (k != null) out.add(k);
      }
    }
    final enc = pack(recommendedEncyclopediaId);
    if (enc != null) out.add(enc);
    final model = modelPack(recommendedModelId);
    if (model != null) out.add(model);
    return out;
  }

  /// Області, для яких у каталозі є карта: (id, назва).
  List<(String, String)> get mapRegions {
    final out = <(String, String)>[];
    for (final p in catalog) {
      final r = p.region;
      if (p.kind != 'map' || r == null) continue;
      out.add((r, regionTitle(p)));
    }
    out.sort((a, b) => a.$2.compareTo(b.$2));
    return out;
  }

  static String regionTitle(PackView p) {
    const prefix = 'Карта: ';
    return p.title.startsWith(prefix)
        ? p.title.substring(prefix.length)
        : p.title;
  }
}

/// Невеликі налаштування завантажень: обрана область і чи показано
/// перший запуск. Звичайний JSON у теці даних.
class DownloadPrefs {
  DownloadPrefs(this.dir);

  final String dir;

  File get _file => File('$dir/downloads.json');

  Map<String, dynamic> _read() {
    try {
      final v = jsonDecode(_file.readAsStringSync());
      if (v is Map<String, dynamic>) return v;
    } catch (_) {}
    return {};
  }

  void _write(Map<String, dynamic> m) {
    try {
      _file.writeAsStringSync(jsonEncode(m));
    } catch (_) {}
  }

  String? get region => _read()['region'] as String?;
  set region(String? v) => _write({..._read(), 'region': v});

  bool get welcomeDone => _read()['welcome_done'] == true;
  void markWelcomeDone() => _write({..._read(), 'welcome_done': true});
}
