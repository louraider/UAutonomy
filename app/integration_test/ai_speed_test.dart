// Замір швидкості локальної моделі на справжньому ПК чи телефоні:
//
//   flutter test integration_test/ai_speed_test.dart -d windows \
//     --dart-define=MODEL_PATH=C:\шлях\до\model.gguf
//
// Друкує рядки `AI_SPEED ...`: бекенд (Vulkan/CPU), шари на GPU, час
// завантаження, першого слова й токени за секунду. Без MODEL_PATH тест
// пропускається (у CI моделі немає).
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:uautonomy/ai/ai_service.dart';
import 'package:uautonomy/src/rust/api/device.dart';
import 'package:uautonomy/src/rust/api/library.dart';
import 'package:uautonomy/src/rust/frb_generated.dart';

const modelPath = String.fromEnvironment('MODEL_PATH');

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets(
    'Швидкість моделі ШІ',
    (tester) async {
      await RustLib.init();
      final dir = await Directory.systemTemp.createTemp('uautonomy-ai');
      await openLibrary(dataDir: dir.path);
      final f = File(modelPath);
      expect(f.existsSync(), isTrue, reason: modelPath);
      final m = LocalModel(
        id: 'speed-test',
        title: f.uri.pathSegments.last,
        path: f.path,
        format: 'gguf',
        sizeBytes: BigInt.from(f.lengthSync()),
        contextSize: 4096,
      );
      final ai = AiService();
      await tester.runAsync(() async {
        await ai.load(m);
        // ignore: avoid_print
        print(
          'AI_SPEED load_s=${(ai.lastLoadTime!.inMilliseconds / 1000).toStringAsFixed(1)} '
          'backend=${ai.backendName} gpu_layers=${ai.gpuLayers} '
          'cores=${Platform.numberOfProcessors} gpu_error=${ai.gpuError}',
        );
        for (final q in [
          'Як підготувати запас води на тиждень для сім\'ї з чотирьох?',
          'Як безпечно користуватися генератором під час блекауту?',
        ]) {
          AiAnswer? last;
          await for (final a in ai.ask(q)) {
            last = a;
          }
          final a = last!;
          // ignore: avoid_print
          print(
            'AI_SPEED q="$q" first_token_ms=${a.firstTokenMs} '
            'total_ms=${a.totalMs} chunks=${a.chunks} tokens=${a.tokens} '
            'chars=${a.text.length} '
            'tok_s=${a.tokensPerSecond?.toStringAsFixed(1)} error=${a.error}',
          );
          expect(a.error, isNull);
          expect(a.text.toString(), isNotEmpty);
        }
        await ai.unload();
      });
    },
    skip: modelPath.isEmpty,
    timeout: const Timeout(Duration(minutes: 15)),
  );
}
