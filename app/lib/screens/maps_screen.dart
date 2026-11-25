import 'package:flutter/foundation.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:latlong2/latlong.dart';
import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/src/rust/api/maps.dart';
import 'package:uautonomy/widgets/brand.dart';
import 'package:vector_map_tiles/vector_map_tiles.dart';
import 'package:vector_tile_renderer/vector_tile_renderer.dart' as vtr;
// ignore: implementation_imports
import 'package:vector_tile_renderer/src/themes/light_theme.dart' as vtr_light;

/// Офлайн-карти: векторні PMTiles (схема OpenMapTiles) за областями.
/// Тайли читає ядро на Rust, малює vector_map_tiles. Інтернет-тайлів
/// немає зовсім, тож карта працює однаково без зв'язку.
class MapsScreen extends StatefulWidget {
  const MapsScreen({
    super.key,
    required this.version,
    required this.onOpenPacks,
  });

  final ValueListenable<int> version;
  final VoidCallback onOpenPacks;

  @override
  State<MapsScreen> createState() => _MapsScreenState();
}

class _MapsScreenState extends State<MapsScreen> {
  List<RegionView> _regions = const [];
  RegionView? _region;
  Future<MapHeader>? _header;
  final _theme = mapTheme();

  /// Колір тла світлої теми OpenMapTiles (шар `background`).
  static const _background = Color(0xFFEFEFEF);

  @override
  void initState() {
    super.initState();
    widget.version.addListener(_reload);
    _reload();
  }

  @override
  void dispose() {
    widget.version.removeListener(_reload);
    super.dispose();
  }

  void _reload() {
    final regions = mapRegions();
    final installed = regions.where((r) => r.mapPath != null).toList();
    setState(() {
      _regions = regions;
      // Лишаємо вибрану область, лише якщо вона встановлена (або нічого не
      // встановлено): після встановлення Києва карта відкривається на Києві,
      // а не на першій у списку невстановленій Вінницькій.
      final keep = _region == null
          ? null
          : (installed.isEmpty ? regions : installed)
                .where((r) => r.id == _region!.id)
                .firstOrNull;
      _select(keep ?? (installed.isNotEmpty ? installed.first : regions.first));
    });
  }

  void _select(RegionView r) {
    _region = r;
    _header = r.mapPath == null ? null : mapHeader(path: r.mapPath!);
  }

  Widget _missing(RegionView? r) => Center(
    child: Padding(
      padding: const EdgeInsets.all(24),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Icon(Icons.map_outlined, size: 48),
          const SizedBox(height: 12),
          Text(
            'Карта області «${r?.title ?? ''}» ще не завантажена. '
            'Після завантаження пакета карта працює без інтернету.',
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 16),
          FilledButton(
            onPressed: widget.onOpenPacks,
            child: const Text('До пакетів'),
          ),
        ],
      ),
    ),
  );

  Widget _map(RegionView r, MapHeader h) {
    // Карти, зібрані tilemaker, не мають меж у заголовку: беремо межі області.
    final b = h.bounds.length == 4 ? h.bounds : r.bbox;
    final bounds = LatLngBounds(LatLng(b[1], b[0]), LatLng(b[3], b[2]));
    return FlutterMap(
      key: ValueKey(r.id),
      options: MapOptions(
        initialCameraFit: CameraFit.bounds(
          bounds: bounds,
          padding: const EdgeInsets.all(8),
        ),
        minZoom: 5,
        maxZoom: 18,
        // Обмежуємо центр, а не весь видимий прямокутник: з
        // CameraConstraint.contain область, меншу за вікно (Київ на ПК),
        // неможливо «вмістити», і карта відкидала будь-який рух чи масштаб.
        cameraConstraint: CameraConstraint.containCenter(bounds: bounds),
        // Поза областю тайлів немає: тло як у теми, а не сіра шахівниця.
        backgroundColor: _background,
        interactionOptions: const InteractionOptions(
          flags: InteractiveFlag.all & ~InteractiveFlag.rotate,
        ),
      ),
      children: [
        VectorTileLayer(
          theme: _theme,
          tileProviders: TileProviders({
            'openmaptiles': _PmTilesProvider(r.mapPath!, h.minZoom, h.maxZoom),
          }),
          maximumZoom: 18,
        ),
        // TextSourceAttribution сам додає «©».
        const RichAttributionWidget(
          attributions: [TextSourceAttribution('учасники OpenStreetMap')],
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    final r = _region;
    return Column(
      children: [
        const TabHeader(title: 'Карти'),
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 0, 16, 0),
          child: DropdownButton<String>(
            isExpanded: true,
            value: r?.id,
            items: [
              for (final x in _regions)
                DropdownMenuItem(
                  value: x.id,
                  child: Text(
                    x.mapPath == null ? '${x.title} (не завантажено)' : x.title,
                  ),
                ),
            ],
            onChanged: (id) =>
                setState(() => _select(_regions.firstWhere((x) => x.id == id))),
          ),
        ),
        Expanded(
          child: r == null || _header == null
              ? _missing(r)
              : FutureBuilder<MapHeader>(
                  key: ValueKey(r.id),
                  future: _header,
                  builder: (context, snap) {
                    if (snap.hasError) {
                      return Center(
                        child: Text('Не вдалося відкрити карту: ${snap.error}'),
                      );
                    }
                    final h = snap.data;
                    if (h == null) {
                      return const Center(child: CircularProgressIndicator());
                    }
                    if (!h.vector) {
                      return const Center(
                        child: Text(
                          'Цей файл карти не векторний (потрібен MVT).',
                        ),
                      );
                    }
                    return _map(r, h);
                  },
                ),
        ),
      ],
    );
  }
}

class _PmTilesProvider extends VectorTileProvider {
  _PmTilesProvider(this.path, this.minimumZoom, this.maximumZoom);

  final String path;

  @override
  final int minimumZoom;

  @override
  final int maximumZoom;

  @override
  TileOffset get tileOffset => TileOffset.DEFAULT;

  @override
  Future<Uint8List> provide(TileIdentity tile) async {
    final data = await mapTile(path: path, z: tile.z, x: tile.x, y: tile.y);
    if (data == null) {
      throw ProviderException(
        message: 'немає тайла ${tile.z}/${tile.x}/${tile.y}',
        statusCode: 404,
        retryable: Retryable.none,
      );
    }
    return data;
  }
}

/// Світла тема OpenMapTiles, але з підписами з `name:latin`: tilemaker
/// кладе туди назву у вихідному письмі (кирилицею), а `name`/`name_en`
/// не заповнює. Без цього карта була б зовсім без назв.
vtr.Theme mapTheme() {
  Object? fix(Object? v) => switch (v) {
    '{name}' || '{name_en}' => '{name:latin}',
    Map<String, dynamic> m => {for (final e in m.entries) e.key: fix(e.value)},
    List<dynamic> l => [for (final x in l) fix(x)],
    _ => v,
  };
  final data = fix(vtr_light.lightThemeData()) as Map<String, dynamic>;
  return vtr.ThemeReader().read(data);
}
