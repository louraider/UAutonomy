import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/services.dart';

/// Номер збірки, який CI вшиває через `--dart-define=UAUTONOMY_BUILD=<номер
/// запуску>`. У локальних збірках 0, і тоді нові версії не шукаємо.
const int currentBuild = int.fromEnvironment('UAUTONOMY_BUILD');

/// Опис останньої збірки, який CI кладе в реліз `app` поруч з APK та
/// інсталятором.
const versionUrl =
    'https://github.com/louraider/UAutonomy/releases/download/app/version.json';

const _channel = MethodChannel('org.uautonomy/app');

/// Нова версія, про яку варто сказати людині.
class UpdateInfo {
  const UpdateInfo({
    required this.build,
    required this.date,
    required this.url,
  });

  final int build;

  /// Дата збірки, `YYYY-MM-DD`, або порожньо.
  final String date;

  /// Сталий короткий шлях на сайті, що веде на потрібний файл.
  final String url;
}

/// Звідки людина взяла застосунок: `site` (APK з сайту, інсталятор
/// Windows, ZIP) або назва магазину (`play`, `msstore`, `appstore`).
/// Збірки для магазинів задають `--dart-define=UAUTONOMY_DIST=<магазин>`.
const String distribution = String.fromEnvironment(
  'UAUTONOMY_DIST',
  defaultValue: 'site',
);

/// Застосунок із сайту ставлять не з магазину, тож про нову версію ніхто,
/// крім нього самого, не скаже. Збірки з магазинів оновлює магазин, а Google
/// Play прямо забороняє оновлюватися повз нього, тож там перевірки немає. На
/// iPhone оновлює App Store.
bool get updateCheckSupported =>
    currentBuild > 0 &&
    distribution == 'site' &&
    (Platform.isAndroid || Platform.isWindows);

/// Розбирає `version.json` і повертає оновлення, якщо воно новіше за
/// [build] і людина не відклала саме його ([dismissed]).
UpdateInfo? parseUpdate(
  String body, {
  required int build,
  required String platformUrl,
  int dismissed = 0,
}) {
  final Object? json;
  try {
    json = jsonDecode(body);
  } on FormatException {
    return null;
  }
  if (json is! Map) return null;
  final remote = json['build'];
  if (remote is! int || remote <= build || remote <= dismissed) return null;
  final date = json['date'];
  return UpdateInfo(
    build: remote,
    date: date is String ? date : '',
    url: platformUrl,
  );
}

/// Посилання на сайті під цю платформу й архітектуру.
Future<String> platformDownloadUrl() async {
  if (Platform.isWindows) return 'https://uautonomy.app/windows';
  String abi = 'arm64-v8a';
  try {
    abi = await _channel.invokeMethod<String>('apkAbi') ?? abi;
  } catch (_) {}
  return switch (abi) {
    'armeabi-v7a' => 'https://uautonomy.app/android-32',
    'x86_64' => 'https://uautonomy.app/android-x86',
    _ => 'https://uautonomy.app/android',
  };
}

File _dismissedFile(String dataDir) =>
    File('$dataDir${Platform.pathSeparator}update-dismissed.txt');

int _readDismissed(String dataDir) {
  try {
    return int.tryParse(_dismissedFile(dataDir).readAsStringSync().trim()) ?? 0;
  } catch (_) {
    return 0;
  }
}

/// Запам'ятати, що про цю збірку більше не нагадувати.
void dismissUpdate(String dataDir, int build) {
  try {
    _dismissedFile(dataDir).writeAsStringSync('$build');
  } catch (_) {}
}

/// Тихо перевіряє, чи вийшла нова версія. Без інтернету чи за будь-якої
/// помилки повертає null: застосунок офлайновий, і це не привід турбувати.
Future<UpdateInfo?> checkForUpdate(String dataDir) async {
  if (!updateCheckSupported) return null;
  final client = HttpClient()..connectionTimeout = const Duration(seconds: 10);
  try {
    final req = await client.getUrl(Uri.parse(versionUrl));
    final resp = await req.close().timeout(const Duration(seconds: 15));
    if (resp.statusCode != 200) return null;
    final body = await resp
        .transform(utf8.decoder)
        .join()
        .timeout(const Duration(seconds: 15));
    return parseUpdate(
      body,
      build: currentBuild,
      platformUrl: await platformDownloadUrl(),
      dismissed: _readDismissed(dataDir),
    );
  } catch (_) {
    return null;
  } finally {
    client.close(force: true);
  }
}

/// Відкриває сторінку завантаження у браузері.
Future<bool> openDownload(String url) async {
  try {
    if (Platform.isWindows) {
      await Process.start('rundll32', ['url.dll,FileProtocolHandler', url]);
      return true;
    }
    return await _channel.invokeMethod<bool>('openUrl', {'url': url}) ?? false;
  } catch (_) {
    return false;
  }
}
