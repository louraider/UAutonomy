// Каталог і ядро без мережі для тестів завантажень.
import 'dart:async';

import 'package:uautonomy/downloads/download_manager.dart';
import 'package:uautonomy/src/rust/api/packs.dart' show PackView, ProgressView;

PackView pack(
  String id,
  String kind, {
  String? region,
  String? modelId,
  int size = 1000,
  bool installed = false,
  String title = '',
}) => PackView(
  id: id,
  title: title.isEmpty ? id : title,
  description: '',
  tier: 'pocket',
  kind: kind,
  version: '1',
  sizeBytes: BigInt.from(size),
  installed: installed,
  region: region,
  modelId: modelId,
);

/// Ядро без мережі: завантаженнями керує тест.
class FakeBackend implements PackBackend {
  FakeBackend(this.catalog);

  List<PackView> catalog;
  Object? catalogError;
  final installedList = <PackView>[];
  final started = <String>[];
  final _active = <String, StreamController<ProgressView>>{};

  @override
  Future<List<PackView>> fetchCatalog(String url) async {
    if (catalogError != null) throw catalogError!;
    return catalog;
  }

  @override
  Stream<ProgressView> download(String id) {
    started.add(id);
    final c = StreamController<ProgressView>();
    _active[id] = c;
    return c.stream;
  }

  void progress(String id, int done, int total) => _active[id]!.add(
    ProgressView(done: BigInt.from(done), total: BigInt.from(total)),
  );

  Future<void> finish(String id) async {
    installedList.add(catalog.firstWhere((p) => p.id == id));
    await _active.remove(id)!.close();
  }

  Future<void> fail(String id, Object e) async {
    final c = _active.remove(id)!;
    c.addError(e);
    await c.close();
  }

  @override
  void cancel() {
    for (final id in _active.keys.toList()) {
      fail(id, 'AnyhowException(скасовано)');
    }
  }

  @override
  List<PackView> installed() => installedList;

  @override
  Future<String?> recommendedModelId() async => 'gemma-e2b';

  @override
  Future<int> freeDiskBytes() async => 10 * 1000 * 1000 * 1000;
}

final sampleCatalog = [
  pack(
    'map-kyivska',
    'map',
    region: 'kyivska',
    title: 'Карта: Київська',
    size: 108089217,
  ),
  pack('map-kyiv', 'map', region: 'kyiv', title: 'Карта: Київ', size: 21373033),
  pack(
    'map-lvivska',
    'map',
    region: 'lvivska',
    title: 'Карта: Львівська',
    size: 69814912,
  ),
  pack(
    recommendedEncyclopediaId,
    'content',
    title: 'Вікіпедія: медицина',
    size: 422928508,
  ),
  pack(
    'zim-wikipedia-uk-top',
    'content',
    title: 'Вікіпедія: найпопулярніші статті',
    size: 2754888077,
  ),
  pack(
    'model-e2b',
    'model',
    modelId: 'gemma-e2b',
    title: 'Gemma 4 E2B (телефон)',
    size: 3106738272,
  ),
  pack(
    'model-12b',
    'model',
    modelId: 'mamay-12b',
    title: 'MamayLM 12B (ПК)',
    size: 7300790491,
  ),
];
