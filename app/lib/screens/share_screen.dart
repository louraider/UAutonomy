import 'dart:io';

import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:uautonomy/screens/common.dart';
import 'package:uautonomy/src/rust/api/share.dart';

const _channel = MethodChannel('org.uautonomy/app');

/// Обмін без інтернету: цей пристрій роздає бібліотеку, пакети й сам
/// застосунок іншим телефонам у тій самій Wi-Fi-мережі чи точці доступу.
class ShareScreen extends StatefulWidget {
  const ShareScreen({super.key});

  @override
  State<ShareScreen> createState() => _ShareScreenState();
}

class _ShareScreenState extends State<ShareScreen> {
  ShareInfo _info = sharingStatus();
  bool _busy = false;

  Future<List<String>> _appFiles() async {
    if (!Platform.isAndroid) return const [];
    try {
      final apk = await _channel.invokeMethod<String>('apkPath');
      if (apk == null) return const [];
      // Віддаємо під зрозумілою назвою.
      final dest = File('${Directory.systemTemp.path}/avtonomiia.apk');
      final src = File(apk);
      // Копіюємо лише раз: APK важить десятки мегабайтів.
      if (!dest.existsSync() || dest.lengthSync() != src.lengthSync()) {
        await src.copy(dest.path);
      }
      return [dest.path];
    } catch (_) {
      return const [];
    }
  }

  Future<void> _toggle() async {
    setState(() => _busy = true);
    try {
      if (_info.running) {
        await stopSharing();
      } else {
        await startSharing(
          deviceName: Platform.localHostname,
          appFiles: await _appFiles(),
        );
      }
      setState(() => _info = sharingStatus());
    } catch (e) {
      if (mounted) showError(context, 'Не вдалося увімкнути обмін: $e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final t = Theme.of(context).textTheme;
    final url = _info.urls.isEmpty ? null : _info.urls.first;
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Text('Поділитися без інтернету', style: t.titleLarge),
        const SizedBox(height: 8),
        const Text(
          '1. Увімкніть точку доступу на цьому телефоні або підключіться до спільного Wi-Fi.\n'
          '2. Натисніть «Увімкнути обмін».\n'
          '3. Інший телефон сканує QR-код камерою й відкриває бібліотеку в браузері, '
          'може завантажити застосунок і пакети. На iPhone і ноутбуках у браузері '
          'працюють пошук, картки й офлайн-карти завантажених областей.',
        ),
        const SizedBox(height: 16),
        FilledButton.icon(
          onPressed: _busy ? null : _toggle,
          icon: Icon(_info.running ? Icons.stop : Icons.wifi_tethering),
          label: Text(_info.running ? 'Вимкнути обмін' : 'Увімкнути обмін'),
        ),
        const SizedBox(height: 24),
        if (_info.running && url == null)
          const Text(
            'Обмін увімкнено, але пристрій не підключений до жодної мережі. '
            'Увімкніть точку доступу або Wi-Fi.',
          ),
        if (url != null) ...[
          Center(
            child: Container(
              color: Colors.white,
              padding: const EdgeInsets.all(12),
              child: QrImageView(data: url, size: 240),
            ),
          ),
          const SizedBox(height: 12),
          Center(child: SelectableText(url, style: t.titleMedium)),
          for (final other in _info.urls.skip(1))
            Center(child: SelectableText(other, style: t.bodySmall)),
          const SizedBox(height: 12),
          const Text(
            'Інший телефон із «Автономією»: «Пакети» → вставте цю адресу → завантажте потрібне.',
          ),
        ],
      ],
    );
  }
}
