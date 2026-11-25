import 'dart:async';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:uautonomy/distribution.dart';
import 'package:uautonomy/screens/common.dart';
import 'package:uautonomy/share/windows_firewall.dart';
import 'package:uautonomy/src/rust/api/share.dart';
import 'package:uautonomy/widgets/brand.dart';

const _channel = MethodChannel('org.uautonomy/app');

/// Обмін без інтернету: цей пристрій роздає бібліотеку, пакети й сам
/// застосунок іншим пристроям у тій самій Wi-Fi-мережі чи точці доступу.
class ShareScreen extends StatefulWidget {
  const ShareScreen({super.key});

  @override
  State<ShareScreen> createState() => _ShareScreenState();
}

class _ShareScreenState extends State<ShareScreen> {
  ShareInfo _info = sharingStatus();
  bool _busy = false;
  FirewallStatus? _firewall;
  bool _fixing = false;

  @override
  void initState() {
    super.initState();
    if (_info.running) _checkFirewall();
  }

  Future<void> _checkFirewall() async {
    if (!Platform.isWindows) return;
    // Запит Windows про дозвіл з'являється, коли сервер починає слухати:
    // даємо людині мить відповісти, а тоді читаємо правила.
    final s = await checkWindowsFirewall();
    if (mounted && _info.running) setState(() => _firewall = s);
  }

  Future<void> _checkFirewallSoon() async {
    await Future<void>.delayed(const Duration(seconds: 2));
    if (mounted && _info.running) await _checkFirewall();
  }

  Future<void> _fixFirewall() async {
    setState(() => _fixing = true);
    final ok = await allowInWindowsFirewall();
    await _checkFirewall();
    if (!mounted) return;
    setState(() => _fixing = false);
    if (!ok) {
      showError(
        context,
        'Брандмауер не змінено. Потрібна згода у вікні Windows '
        '(обліковий запис адміністратора).',
      );
    }
  }

  static const _blockedText =
      'Брандмауер Windows не пускає інші пристрої до обміну: телефон не '
      'відкриє сторінку.';

  Widget _firewallNote(BuildContext context) {
    final f = _firewall;
    if (f == null || f.verdict == FirewallVerdict.ok) {
      return const SizedBox.shrink();
    }
    final blocked = f.verdict == FirewallVerdict.blocked;
    final where = f.publicNetwork
        ? 'Ця мережа позначена у Windows як загальнодоступна. '
        : '';
    return Card(
      color: Theme.of(context).colorScheme.errorContainer,
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              blocked
                  ? '$where$_blockedText'
                  : 'Брандмауер Windows ще не має дозволу для обміну. Якщо '
                        "з'явилося вікно із запитом, позначте обидві галочки "
                        '(приватні й загальнодоступні мережі).',
            ),
            const SizedBox(height: 8),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                FilledButton(
                  onPressed: _fixing ? null : _fixFirewall,
                  child: Text(
                    _fixing
                        ? 'Чекаю на згоду…'
                        : 'Дозволити в локальній мережі',
                  ),
                ),
                OutlinedButton(
                  onPressed: _fixing ? null : _checkFirewall,
                  child: const Text('Перевірити ще раз'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Future<List<String>> _appFiles() async {
    // Із Google Play застосунок ставиться частинами (AAB), і один APK з
    // телефона на іншому не запрацює. До того ж Play забороняє роздавати
    // застосунок повз нього, тому в магазинній збірці APK не роздаємо.
    if (!Platform.isAndroid || isStoreBuild) return const [];
    try {
      final apk = await _channel.invokeMethod<String>('apkPath');
      if (apk == null) return const [];
      // Віддаємо під зрозумілою назвою з архітектурою: APK збирається
      // окремо для arm64-v8a, armeabi-v7a і x86_64, а сторінка обміну
      // підказує за назвою, яким телефонам він підійде.
      final abi = await _channel.invokeMethod<String>('apkAbi') ?? 'android';
      final dest = File('${Directory.systemTemp.path}/avtonomiia-$abi.apk');
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
      setState(() {
        _info = sharingStatus();
        _firewall = null;
      });
      // iPhone призупиняє застосунок, коли гасне екран, і сервер обміну
      // перестає відповідати. Поки роздаємо, не даємо екрану згаснути.
      if (Platform.isIOS) {
        unawaited(
          _channel
              .invokeMethod<void>('keepAwake', {'on': _info.running})
              .catchError((_) {}),
        );
      }
      if (_info.running) {
        // Не тримаємо кнопку вимкненою, поки PowerShell читає правила.
        unawaited(_checkFirewallSoon());
      }
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
    return ContentWidth(
      child: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Text('Обмін', style: t.headlineMedium),
          const SizedBox(height: 4),
          Text('Поділитися без інтернету', style: t.titleMedium),
          const SizedBox(height: 12),
          Text(
            Platform.isIOS
                ? '1. Увімкніть на цьому iPhone «Режим модема» або підключіться до спільного Wi-Fi.\n'
                      '2. Натисніть «Увімкнути обмін» і не закривайте застосунок: iPhone зупиняє '
                      'обмін, щойно застосунок іде у фон. Поки обмін увімкнено, екран не гасне.\n'
                      '3. Інший телефон сканує QR-код камерою, відкриває бібліотеку в браузері '
                      'і може завантажити пакети. У браузері працюють пошук, картки й офлайн-карти '
                      'завантажених областей. Сам застосунок для Android можна отримати лише з Android-телефона чи комп\'ютера.'
                : Platform.isAndroid
                ? '1. Увімкніть точку доступу на цьому телефоні або підключіться до спільного Wi-Fi.\n'
                      '2. Натисніть «Увімкнути обмін».\n'
                      '3. Інший телефон сканує QR-код камерою, відкриває бібліотеку в браузері '
                      '${isStoreBuild ? 'і може завантажити пакети' : 'і може завантажити застосунок і пакети'}. На iPhone і ноутбуках у браузері '
                      'працюють пошук, картки й офлайн-карти завантажених областей.'
                : '1. Підключіть цей комп\'ютер і телефон до одного Wi-Fi чи роутера '
                      '(або підключіть комп\'ютер до точки доступу телефона).\n'
                      '2. Натисніть «Увімкнути обмін». Якщо Windows запитає дозвіл для '
                      'брандмауера, позначте обидві галочки (приватні й загальнодоступні '
                      'мережі), інакше телефон може не відкрити сторінку.\n'
                      '3. Телефон сканує QR-код камерою, відкриває бібліотеку в браузері '
                      'і може завантажити застосунок і пакети. На iPhone у браузері працюють '
                      'пошук, картки й офлайн-карти завантажених областей.',
          ),
          const SizedBox(height: 16),
          FilledButton.icon(
            onPressed: _busy ? null : _toggle,
            icon: Icon(_info.running ? Icons.stop : Icons.wifi_tethering),
            label: Text(_info.running ? 'Вимкнути обмін' : 'Увімкнути обмін'),
          ),
          if (_info.running && _firewall != null) ...[
            const SizedBox(height: 16),
            _firewallNote(context),
          ],
          const SizedBox(height: 24),
          if (_info.running && url == null)
            const Text(
              'Обмін увімкнено, але пристрій не підключений до жодної мережі. '
              'Увімкніть точку доступу, Wi-Fi чи кабель.',
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
              'Якщо QR-код не відкривається, спробуйте іншу адресу зі списку. Найімовірніше '
              'спрацює перша, віртуальні адаптери (WSL, VPN) стоять у кінці.\n'
              'На іншому пристрої з «Автономією» відкрийте вкладку «Пакети», вставте цю адресу '
              'в поле «Адреса каталогу або іншого пристрою», натисніть стрілку і завантажте потрібне.',
            ),
          ],
        ],
      ),
    );
  }
}
