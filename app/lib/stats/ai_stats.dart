import 'package:uautonomy/ai/ai_service.dart';
import 'package:uautonomy/stats/stats.dart';

/// Чим закінчилось запитання до помічника. Тексту запитання й відповіді
/// тут немає: лише результат, тема маршруту і кількість джерел.
String answerOutcome(AiAnswer a) {
  final route = a.route;
  if (route != null) return route.cards.isEmpty ? 'no_card' : 'cards';
  final error = a.error;
  if (error != null) {
    return error == 'Модель не завантажено' ? 'no_model' : 'error';
  }
  if (a.sources.isEmpty) return 'no_sources';
  if (saysNotInLibrary(a.text.toString())) return 'not_in_library';
  return 'answered';
}

/// Модель за інструкцією каже прямо, що в бібліотеці цього немає.
bool saysNotInLibrary(String text) {
  final t = text.toLowerCase();
  return t.contains('бібліотец') &&
      (t.contains('немає') || t.contains('нема ') || t.contains('не знайш'));
}

void trackAnswer(AiAnswer a) => track('ai_question', {
  'outcome': answerOutcome(a),
  'mode': a.route == null ? 'model' : 'cards',
  'topic': ?a.route?.topic,
  'sources': a.sources.length,
});
