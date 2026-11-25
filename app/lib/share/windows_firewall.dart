// Чи дійде телефон до обміну на Windows: профіль мережі й правила
// брандмауера для цього exe.
//
// Коли програма вперше слухає порт, Windows питає дозвіл і за замовчуванням
// ставить галочку лише «Приватні мережі». А нова Wi-Fi чи кабельна мережа
// часто позначена як «Загальнодоступна». Тоді Windows створює правило
// «Блокувати» для загальнодоступних мереж, і телефон мовчки не відкриває
// сторінку. Тут ми це помічаємо й пропонуємо виправити одним дозволом
// адміністратора.
import 'dart:convert';
import 'dart:io';

/// Вхідне правило брандмауера для нашого exe.
class FirewallRule {
  const FirewallRule({required this.allow, required this.profile});

  final bool allow;

  /// Як у PowerShell: `Any`, `Public`, `Private`, `Domain` або перелік через кому.
  final String profile;

  bool covers(String category) {
    final p = profile.toLowerCase();
    if (p.contains('any')) return true;
    final c = category.toLowerCase().startsWith('domain')
        ? 'domain'
        : category.toLowerCase();
    return p.split(',').map((s) => s.trim()).contains(c);
  }
}

enum FirewallVerdict {
  /// Правило дозволяє вхідні в усіх підключених мережах.
  ok,

  /// Правило «Блокувати» для однієї з підключених мереж: телефон не дійде.
  blocked,

  /// Правил для нашого exe ще немає: Windows спитає (або мовчки заблокує).
  unknown,
}

class FirewallStatus {
  const FirewallStatus(this.verdict, this.categories);

  final FirewallVerdict verdict;

  /// Категорії підключених мереж: `Public`, `Private`, `DomainAuthenticated`.
  final List<String> categories;

  bool get publicNetwork => categories.any((c) => c.toLowerCase() == 'public');
}

/// Чиста логіка (перевіряється тестом): блокування сильніше за дозвіл, як у
/// самому брандмауері Windows.
FirewallVerdict assessFirewall(
  List<String> categories,
  List<FirewallRule> rules,
) {
  if (categories.isEmpty) return FirewallVerdict.ok;
  for (final c in categories) {
    if (rules.any((r) => !r.allow && r.covers(c))) {
      return FirewallVerdict.blocked;
    }
  }
  final allAllowed = categories.every(
    (c) => rules.any((r) => r.allow && r.covers(c)),
  );
  if (allAllowed) return FirewallVerdict.ok;
  return rules.isEmpty ? FirewallVerdict.unknown : FirewallVerdict.blocked;
}

String _psQuote(String s) => "'${s.replaceAll("'", "''")}'";

/// Читає профіль мережі й правила для [exe]. Права адміністратора не
/// потрібні. `null`, якщо не Windows чи PowerShell недоступний.
Future<FirewallStatus?> checkWindowsFirewall({String? exe}) async {
  if (!Platform.isWindows) return null;
  final program = exe ?? Platform.resolvedExecutable;
  final script =
      r'$ErrorActionPreference="SilentlyContinue";'
      r'$c=@(Get-NetConnectionProfile|ForEach-Object{"$($_.NetworkCategory)"});'
      '\$r=@(Get-NetFirewallApplicationFilter -ErrorAction SilentlyContinue -Program ${_psQuote(program)}|'
      r'Get-NetFirewallRule|Where-Object{$_.Direction -eq "Inbound" -and $_.Enabled -eq "True"}|'
      r'ForEach-Object{@{a="$($_.Action)";p="$($_.Profile)"}});'
      r'@{categories=$c;rules=$r}|ConvertTo-Json -Compress -Depth 3';
  try {
    final res = await Process.run('powershell', [
      '-NoProfile',
      '-NonInteractive',
      '-Command',
      script,
    ]).timeout(const Duration(seconds: 20));
    // Для exe без правил Get-NetFirewallApplicationFilter пише помилку в
    // stderr, але JSON однаково друкує: дивимося лише на stdout.
    final out = (res.stdout as String).trim();
    if (out.isEmpty) return null;
    final j = jsonDecode(out) as Map<String, dynamic>;
    List<dynamic> list(Object? v) => v == null
        ? const []
        : v is List
        ? v
        : [v];
    final categories = list(j['categories']).map((e) => '$e').toList();
    final rules = list(j['rules'])
        .cast<Map<String, dynamic>>()
        .map(
          (r) => FirewallRule(
            allow: '${r['a']}'.toLowerCase() == 'allow',
            profile: '${r['p']}',
          ),
        )
        .toList();
    return FirewallStatus(assessFirewall(categories, rules), categories);
  } catch (_) {
    return null;
  }
}

/// Просить дозвіл адміністратора (вікно UAC) і дозволяє вхідні TCP для
/// цього exe лише з локальної мережі; правила «Блокувати» для нього
/// прибирає. `true`, якщо після цього перевірка показує «гаразд».
Future<bool> allowInWindowsFirewall({String? exe}) async {
  if (!Platform.isWindows) return false;
  final program = _psQuote(exe ?? Platform.resolvedExecutable);
  final inner =
      'Get-NetFirewallApplicationFilter -Program $program|Get-NetFirewallRule|'
      'Where-Object{\$_.Direction -eq "Inbound" -and \$_.Action -eq "Block"}|Remove-NetFirewallRule;'
      'New-NetFirewallRule -DisplayName "Автономія: обмін у локальній мережі" '
      '-Direction Inbound -Action Allow -Program $program -Protocol TCP '
      '-Profile Any -RemoteAddress LocalSubnet|Out-Null';
  final encoded = base64.encode(
    inner.codeUnits.expand((u) => [u & 0xff, u >> 8]).toList(),
  );
  try {
    await Process.run('powershell', [
      '-NoProfile',
      '-NonInteractive',
      '-Command',
      'Start-Process powershell -Verb RunAs -Wait -WindowStyle Hidden '
          '-ArgumentList "-NoProfile","-EncodedCommand","$encoded"',
    ]).timeout(const Duration(minutes: 3));
  } catch (_) {
    return false;
  }
  return (await checkWindowsFirewall(exe: exe))?.verdict == FirewallVerdict.ok;
}
