import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_map_pmtiles/flutter_map_pmtiles.dart';
import 'package:latlong2/latlong.dart';
import 'package:flutter/foundation.dart';
import 'package:material_ui/material_ui.dart';
import 'package:uautonomy/src/rust/api/maps.dart';

/// Офлайн-карти: растрові PMTiles за областями. Інтернет-тайли не
/// використовуємо зовсім, щоб карта працювала однаково без зв'язку.
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
  Future<PmTilesTileProvider>? _provider;
  final _map = MapController();

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
      final keep = _region == null
          ? null
          : regions.where((r) => r.id == _region!.id).firstOrNull;
      _select(keep ?? (installed.isNotEmpty ? installed.first : regions.first));
    });
  }

  void _select(RegionView r) {
    _region = r;
    _provider = r.mapPath == null
        ? null
        : PmTilesTileProvider.fromSource(r.mapPath!);
  }

  @override
  Widget build(BuildContext context) {
    final r = _region;
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
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
          child: r == null || _provider == null
              ? Center(
                  child: Padding(
                    padding: const EdgeInsets.all(24),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        const Icon(Icons.map_outlined, size: 48),
                        const SizedBox(height: 12),
                        Text(
                          'Карта області «${r?.title ?? ''}» ще не завантажена. '
                          'Карти працюють без інтернету після завантаження пакета.',
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
                )
              : FutureBuilder<PmTilesTileProvider>(
                  key: ValueKey(r.id),
                  future: _provider,
                  builder: (context, snap) {
                    if (snap.hasError) {
                      return Center(
                        child: Text('Не вдалося відкрити карту: ${snap.error}'),
                      );
                    }
                    if (!snap.hasData) {
                      return const Center(child: CircularProgressIndicator());
                    }
                    final b = r.bbox;
                    return FlutterMap(
                      mapController: _map,
                      options: MapOptions(
                        initialCenter: LatLng(r.centerLat, r.centerLon),
                        initialZoom: 9,
                        minZoom: 5,
                        maxZoom: 16,
                        cameraConstraint: CameraConstraint.contain(
                          bounds: LatLngBounds(
                            LatLng(b[1], b[0]),
                            LatLng(b[3], b[2]),
                          ),
                        ),
                      ),
                      children: [
                        TileLayer(tileProvider: snap.data!, maxNativeZoom: 14),
                        const RichAttributionWidget(
                          attributions: [
                            TextSourceAttribution('© учасники OpenStreetMap'),
                          ],
                        ),
                      ],
                    );
                  },
                ),
        ),
      ],
    );
  }
}
