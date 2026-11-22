import 'dart:async';

import 'package:llamadart/llamadart.dart';
import 'package:uautonomy/src/rust/api/ai.dart';
import 'package:uautonomy/src/rust/api/device.dart';

/// Одна відповідь помічника: текст, що надходить потоком, і джерела.
class AiAnswer {
  AiAnswer(this.question, this.sources);

  final String question;
  final List<SourceView> sources;
  final StringBuffer text = StringBuffer();
  bool done = false;
  String? error;
}

/// Обгортка над llamadart. Модель змінна: будь-який файл GGUF чи
/// LiteRT-LM з [localModels] (Gemma 4, MamayLM, Lapa, власна).
class AiService {
  LlamaEngine? _engine;
  LocalModel? _model;
  StreamSubscription<LlamaCompletionChunk>? _sub;
  AiAnswer? _current;

  LocalModel? get model => _model;
  bool get isLoaded => _engine != null;

  Future<void> load(LocalModel m) async {
    if (_model?.path == m.path && _engine != null) return;
    await unload();
    _engine = await LlamaEngine.load(
      LlamaModel(ModelSource.path(m.path)),
      params: ModelParams(contextSize: m.contextSize),
    );
    _model = m;
  }

  Future<void> unload() async {
    await stop();
    final e = _engine;
    _engine = null;
    _model = null;
    await e?.dispose();
  }

  /// Ставить запитання. Кожне запитання самостійне: модель бачить лише
  /// фрагменти бібліотеки, знайдені саме під нього (так менше вигадок).
  Stream<AiAnswer> ask(String question) {
    final controller = StreamController<AiAnswer>();
    _ask(question, controller);
    return controller.stream;
  }

  Future<void> _ask(
    String question,
    StreamController<AiAnswer> controller,
  ) async {
    final engine = _engine;
    final rag = await buildRagPrompt(
      question: question,
      contextTokens: _model?.contextSize ?? 4096,
    );
    final answer = AiAnswer(question, rag.sources);
    if (engine == null) {
      answer
        ..error = 'Модель не завантажено'
        ..done = true;
      controller
        ..add(answer)
        ..close();
      return;
    }
    final messages = [
      LlamaChatMessage.fromText(role: LlamaChatRole.system, text: rag.system),
      LlamaChatMessage.fromText(role: LlamaChatRole.user, text: rag.user),
    ];
    controller.add(answer);
    _current = answer;
    _sub = engine
        .create(
          messages,
          params: const GenerationParams(maxTokens: 700, temp: 0.3, topP: 0.9),
          enableThinking: false,
        )
        .listen(
          (chunk) {
            answer.text.write(chunk.text);
            controller.add(answer);
          },
          onError: (Object e) {
            answer
              ..error = '$e'
              ..done = true;
            controller
              ..add(answer)
              ..close();
          },
          onDone: () {
            answer.done = true;
            controller
              ..add(answer)
              ..close();
          },
          cancelOnError: true,
        );
  }

  Future<void> stop() async {
    await _sub?.cancel();
    _sub = null;
    _current?.done = true;
    _current = null;
  }
}
