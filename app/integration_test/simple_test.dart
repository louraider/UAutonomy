import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:uautonomy/main.dart';
import 'package:uautonomy/src/rust/api/library.dart';
import 'package:uautonomy/src/rust/frb_generated.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    await RustLib.init();
    final dir = await Directory.systemTemp.createTemp('uautonomy-test');
    await openLibrary(dataDir: dir.path);
  });

  testWidgets('Бібліотека відкривається, пошук працює офлайн', (tester) async {
    await tester.pumpWidget(const UAutonomyApp());
    await tester.pumpAndSettle();
    expect(find.text('Довідка'), findsOneWidget);
    expect(search(query: 'тривожна валіза'), isNotEmpty);
    expect(coreVersion(), isNotEmpty);
  });
}
