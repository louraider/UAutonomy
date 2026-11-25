import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:uautonomy/share/windows_firewall.dart';

const allowPrivate = FirewallRule(allow: true, profile: 'Private');
const blockPublic = FirewallRule(allow: false, profile: 'Public');

void main() {
  test('типовий запит Windows у загальнодоступній мережі блокує обмін', () {
    // Галочка лише «Приватні»: Windows дозволяє Private і блокує Public.
    expect(
      assessFirewall(['Public'], [allowPrivate, blockPublic]),
      FirewallVerdict.blocked,
    );
    expect(
      assessFirewall(['Private'], [allowPrivate, blockPublic]),
      FirewallVerdict.ok,
    );
  });

  test('дозвіл для загальнодоступних чи будь-яких мереж', () {
    // Так на ПК розробника: два правила Public Allow (TCP і UDP).
    expect(
      assessFirewall(['Public'], const [
        FirewallRule(allow: true, profile: 'Public'),
      ]),
      FirewallVerdict.ok,
    );
    expect(
      assessFirewall(['Public', 'DomainAuthenticated'], const [
        FirewallRule(allow: true, profile: 'Any'),
      ]),
      FirewallVerdict.ok,
    );
    expect(
      assessFirewall(['DomainAuthenticated'], const [
        FirewallRule(allow: true, profile: 'Domain, Private'),
      ]),
      FirewallVerdict.ok,
    );
  });

  test('немає правил: Windows ще спитає', () {
    expect(assessFirewall(['Public'], const []), FirewallVerdict.unknown);
    expect(assessFirewall(['Public'], [allowPrivate]), FirewallVerdict.blocked);
    expect(assessFirewall(const [], const []), FirewallVerdict.ok);
  });

  test('блокування сильніше за дозвіл', () {
    expect(
      assessFirewall(['Private'], const [
        FirewallRule(allow: true, profile: 'Any'),
        FirewallRule(allow: false, profile: 'Private'),
      ]),
      FirewallVerdict.blocked,
    );
  });

  test(
    'на Windows справді читає профіль мережі й правила',
    () async {
      const exe = String.fromEnvironment('FIREWALL_EXE');
      final s = await checkWindowsFirewall(
        exe: exe.isEmpty ? Platform.resolvedExecutable : exe,
      );
      expect(s, isNotNull);
      // ignore: avoid_print
      print('FIREWALL ${s!.verdict} ${s.categories}');
      expect(s.categories, isNotEmpty);
    },
    skip: !Platform.isWindows,
  );
}
