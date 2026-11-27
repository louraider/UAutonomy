/// Анонімна статистика використання, лише за згодою людини.
///
/// Події йдуть в Aptabase (aptabase.com, відкритий код, сервери в ЄС): він
/// не ставить ідентифікатора пристрою, не зберігає IP-адресу і показує лише
/// загальні числа. Ключ задається під час збірки:
///
///   --dart-define=UAUTONOMY_STATS_KEY=A-EU-0000000000
///
/// Ключ виду A-SH-… означає власний сервер Aptabase, тоді потрібна ще
/// адреса: --dart-define=UAUTONOMY_STATS_HOST=https://stats.example.org
///
/// Без ключа статистика вимкнена повністю: застосунок не питає згоди і
/// нічого не записує.
///
/// Застосунок працює без інтернету, тому події спершу лягають у файл у
/// теці застосунку і надсилаються пачками, коли є мережа. Що саме
/// надсилається, видно в [StatsEvent] і в описі на екрані налаштувань.
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';

import 'package:flutter/foundation.dart';
import 'package:uautonomy/distribution.dart';

const String statsKey = String.fromEnvironment('UAUTONOMY_STATS_KEY');
const String _statsHost = String.fromEnvironment('UAUTONOMY_STATS_HOST');

/// Редакція тексту згоди. Якщо склад статистики суттєво зміниться,
/// збільште її: застосунок спитає ще раз.
const int statsConsentVersion = 1;

/// Адреса прийому подій або null, якщо ключ не задано чи він хибний.
Uri? statsEndpoint(String key, {String host = _statsHost}) {
  final parts = key.split('-');
  if (parts.length != 3 || parts[0] != 'A' || parts[2].isEmpty) return null;
  final base = switch (parts[1]) {
    'EU' => 'https://eu.aptabase.com',
    'US' => 'https://us.aptabase.com',
    'SH' => host,
    _ => '',
  };
  if (base.isEmpty || !base.startsWith('https://')) return null;
  return Uri.parse('$base/api/v0/events');
}

/// Чи є в цій збірці куди надсилати статистику.
bool get statsAvailable => statsEndpoint(statsKey) != null;

/// Рішення людини щодо статистики.
enum StatsConsent { unknown, yes, no }

/// Одна подія в черзі.
class StatsEvent {
  StatsEvent(this.name, this.props, this.time);

  final String name;
  final Map<String, Object> props;
  final DateTime time;

  Map<String, Object?> toJson(String sessionId) => {
    'timestamp': time.toUtc().toIso8601String(),
    'sessionId': sessionId,
    'eventName': name,
    'systemProps': _systemProps,
    'props': props,
  };

  Map<String, Object> toStore() => {
    'n': name,
    'p': props,
    't': time.toUtc().millisecondsSinceEpoch,
  };

  static StatsEvent? fromStore(Object? j) {
    if (j is! Map) return null;
    final n = j['n'], p = j['p'], t = j['t'];
    if (n is! String || p is! Map || t is! int) return null;
    return StatsEvent(n, {
      for (final e in p.entries)
        if (e.value is String || e.value is num) '${e.key}': e.value as Object,
    }, DateTime.fromMillisecondsSinceEpoch(t, isUtc: true));
  }
}

/// Лише система, мова і номер збірки. Без моделі пристрою, без версії ОС
/// (на Android це довгий рядок, який допомагає впізнати телефон).
final Map<String, Object> _systemProps = {
  'isDebug': kDebugMode,
  'osName': switch (Platform.operatingSystem) {
    'android' => 'Android',
    'ios' => 'iOS',
    'windows' => 'Windows',
    'macos' => 'macOS',
    'linux' => 'Linux',
    final o => o,
  },
  'locale': 'uk',
  'appVersion': const String.fromEnvironment(
    'UAUTONOMY_BUILD',
    defaultValue: '0',
  ),
  'appBuildNumber': distChannel,
  'sdkVersion': 'uautonomy@1',
};

/// Надсилає пачку подій. Повертає код відповіді або null без мережі.
typedef StatsSender = Future<int?> Function(Uri url, String key, String body);

Future<int?> _httpSend(Uri url, String key, String body) async {
  final client = HttpClient()..connectionTimeout = const Duration(seconds: 15);
  try {
    final req = await client.postUrl(url);
    req.headers
      ..set('App-Key', key)
      ..set(HttpHeaders.contentTypeHeader, 'application/json; charset=utf-8')
      ..set(HttpHeaders.userAgentHeader, 'uautonomy');
    req.add(utf8.encode(body));
    final res = await req.close().timeout(const Duration(seconds: 30));
    await res.drain<void>();
    return res.statusCode;
  } catch (_) {
    return null;
  } finally {
    client.close(force: true);
  }
}

class Stats {
  Stats({
    this.key = statsKey,
    Uri? endpoint,
    StatsSender? sender,
    this.flushDelay = const Duration(seconds: 30),
    this.retryEvery = const Duration(minutes: 10),
  }) : endpoint = endpoint ?? statsEndpoint(key),
       _sender = sender ?? _httpSend;

  /// Спільний екземпляр для застосунку.
  static final Stats instance = Stats();

  final String key;
  final Uri? endpoint;
  final StatsSender _sender;
  final Duration flushDelay;
  final Duration retryEvery;

  /// Скільки подій може чекати на мережу. Старіші за [maxAge] відкидаються.
  static const int maxQueue = 1000;
  static const Duration maxAge = Duration(days: 30);
  static const int batchSize = 25;

  /// Випадковий номер сеансу, новий на кожному запуску і щогодини. Його не
  /// зберігаємо, тож різні запуски однієї людини не пов'язати між собою.
  String _session = _newSession();
  DateTime _sessionStart = DateTime.now();

  String? _dir;
  StatsConsent _consent = StatsConsent.unknown;
  final List<StatsEvent> _queue = [];
  Timer? _flushTimer;
  Timer? _retryTimer;
  bool _sending = false;

  bool get available => endpoint != null;
  StatsConsent get consent => _consent;
  bool get enabled => available && _consent == StatsConsent.yes;

  /// Чи треба спитати людину про згоду.
  bool get needsConsent => available && _consent == StatsConsent.unknown;

  @visibleForTesting
  List<StatsEvent> get queue => List.unmodifiable(_queue);

  File _consentFile(String dir) => File('$dir/stats_consent');
  File _queueFile(String dir) => File('$dir/stats_queue.json');

  Future<void> init(String dataDir) async {
    _dir = dataDir;
    _consent = await _readConsent(dataDir);
    if (!enabled) {
      // Згоду не дали чи відкликали: на диску не лишаємо нічого.
      await _deleteQueue();
      return;
    }
    await _loadQueue();
    _retryTimer?.cancel();
    _retryTimer = Timer.periodic(retryEvery, (_) => flush());
    unawaited(flush());
  }

  Future<StatsConsent> _readConsent(String dir) async {
    try {
      final f = _consentFile(dir);
      if (!await f.exists()) return StatsConsent.unknown;
      final parts = (await f.readAsString()).trim().split(' ');
      if (parts.length != 2 || int.tryParse(parts[1]) != statsConsentVersion) {
        return StatsConsent.unknown;
      }
      return parts[0] == 'yes' ? StatsConsent.yes : StatsConsent.no;
    } catch (_) {
      return StatsConsent.unknown;
    }
  }

  /// Записує рішення людини. «Ні» одразу стирає чергу.
  Future<void> setConsent(bool yes) async {
    _consent = yes ? StatsConsent.yes : StatsConsent.no;
    final dir = _dir;
    if (dir != null) {
      try {
        final f = _consentFile(dir);
        await f.parent.create(recursive: true);
        await f.writeAsString('${yes ? 'yes' : 'no'} $statsConsentVersion');
      } catch (_) {}
    }
    if (yes) {
      _retryTimer ??= Timer.periodic(retryEvery, (_) => flush());
    } else {
      _flushTimer?.cancel();
      _retryTimer?.cancel();
      _retryTimer = null;
      _queue.clear();
      await _saving;
      await _deleteQueue();
    }
  }

  /// Записує подію, якщо людина погодилась. Значення лише рядки й числа.
  void track(String name, [Map<String, Object> props = const {}]) {
    if (!enabled) return;
    _queue.add(StatsEvent(name, Map.of(props), DateTime.now()));
    if (_queue.length > maxQueue) {
      _queue.removeRange(0, _queue.length - maxQueue);
    }
    unawaited(_saveQueue());
    _flushTimer?.cancel();
    _flushTimer = Timer(flushDelay, flush);
  }

  /// Надсилає чергу пачками. Без мережі події лишаються до наступної спроби.
  Future<void> flush() async {
    final url = endpoint;
    if (!enabled || url == null || _sending) return;
    _sending = true;
    try {
      _dropOld();
      while (_queue.isNotEmpty && enabled) {
        if (DateTime.now().difference(_sessionStart) >
            const Duration(hours: 1)) {
          _session = _newSession();
          _sessionStart = DateTime.now();
        }
        final batch = _queue.take(batchSize).toList();
        final body = jsonEncode([for (final e in batch) e.toJson(_session)]);
        final code = await _sender(url, key, body);
        // Немає мережі чи сервер не відповідає: спробуємо пізніше.
        if (code == null || code >= 500 || code == 429) break;
        // Прийнято, або сервер відкинув пачку як хибну: в обох випадках
        // повторювати її немає сенсу.
        _queue.removeRange(0, batch.length);
        await _saveQueue();
      }
    } finally {
      _sending = false;
    }
  }

  void _dropOld() {
    final cutoff = DateTime.now().subtract(maxAge);
    _queue.removeWhere((e) => e.time.isBefore(cutoff));
  }

  Future<void> _loadQueue() async {
    final dir = _dir;
    if (dir == null) return;
    try {
      final f = _queueFile(dir);
      if (!await f.exists()) return;
      final data = jsonDecode(await f.readAsString());
      if (data is! List) return;
      _queue
        ..clear()
        ..addAll(data.map(StatsEvent.fromStore).whereType<StatsEvent>());
      _dropOld();
    } catch (_) {
      _queue.clear();
    }
  }

  Future<void> _saving = Future.value();

  /// Записи йдуть по черзі, щоб два одночасні не зіпсували файл.
  Future<void> _saveQueue() => _saving = _saving.then((_) => _writeQueue());

  Future<void> _writeQueue() async {
    final dir = _dir;
    if (dir == null) return;
    try {
      final f = _queueFile(dir);
      if (_queue.isEmpty) {
        if (await f.exists()) await f.delete();
        return;
      }
      await f.parent.create(recursive: true);
      final tmp = File('${f.path}.tmp');
      await tmp.writeAsString(
        jsonEncode([for (final e in _queue) e.toStore()]),
      );
      await tmp.rename(f.path);
    } catch (_) {}
  }

  Future<void> _deleteQueue() async {
    final dir = _dir;
    if (dir == null) return;
    try {
      final f = _queueFile(dir);
      if (await f.exists()) await f.delete();
    } catch (_) {}
  }

  @visibleForTesting
  void dispose() {
    _flushTimer?.cancel();
    _retryTimer?.cancel();
  }
}

/// Формат, як у SDK Aptabase: секунди від 1970 року і 8 випадкових цифр.
String _newSession() {
  final secs = DateTime.now().millisecondsSinceEpoch ~/ 1000;
  final rnd = Random.secure().nextInt(100000000).toString().padLeft(8, '0');
  return '$secs$rnd';
}

/// Скорочення для подій: `track('tab_open', {'tab': 'maps'})`.
void track(String name, [Map<String, Object> props = const {}]) =>
    Stats.instance.track(name, props);
