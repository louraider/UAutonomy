import 'package:flutter_test/flutter_test.dart';
import 'package:uautonomy/main.dart';
import 'package:uautonomy/src/rust/frb_generated.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async => await RustLib.init());
  testWidgets('Застосунок показує версію ядра з Rust', (tester) async {
    await tester.pumpWidget(const UAutonomyApp());
    expect(find.textContaining('ядро 0.1.0'), findsOneWidget);
  });
}
