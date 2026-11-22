//! Підготовка запиту до локальної моделі з опорою на бібліотеку (RAG).
//! Сама модель працює на боці застосунку (llamadart), ядро лише підбирає
//! фрагменти й складає інструкції, тож модель легко замінити.

use serde::Serialize;

use crate::search::{Library, Passage};

pub const SYSTEM_PROMPT: &str = "Ти — помічник офлайн-бібліотеки «Автономія» для людей в Україні під час війни та надзвичайних ситуацій. \
Відповідай українською, коротко, нумерованими кроками. \
Спирайся ЛИШЕ на наведені нижче фрагменти бібліотеки й позначай джерело номером у квадратних дужках, напр. [1]. \
Якщо у фрагментах немає відповіді, прямо скажи, що в бібліотеці цього немає, і не вигадуй. \
Не вигадуй дозувань ліків, номерів телефонів чи адрес. \
Якщо є загроза життю, першим рядком порадь викликати 112 або 103 (якщо є зв'язок). \
Не давай порад щодо поводження з вибухонебезпечними предметами, окрім «не торкатися, відійти, позначити місце, повідомити 101 або 112».";

/// Готовий запит: системна інструкція, повідомлення користувача з
/// контекстом і список використаних джерел для показу під відповіддю.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RagPrompt {
    pub system: String,
    pub user: String,
    pub passages: Vec<Passage>,
}

/// `max_context_chars` обмежує обсяг фрагментів під контекст моделі
/// (≈ 3–4 символи на токен для української).
pub fn build_prompt(
    lib: &Library,
    question: &str,
    k: usize,
    max_context_chars: usize,
) -> RagPrompt {
    let mut passages = lib.retrieve(question, k);
    let mut ctx = String::new();
    let mut used = Vec::new();
    for p in passages.drain(..) {
        let block = format!(
            "[{}] {} (джерело: {}{})\n{}\n\n",
            used.len() + 1,
            p.title,
            p.source,
            if p.reviewed {
                ""
            } else {
                "; не перевірено фахівцем"
            },
            p.text.trim()
        );
        if !used.is_empty() && ctx.chars().count() + block.chars().count() > max_context_chars {
            break;
        }
        ctx.push_str(&block);
        used.push(p);
    }
    let user = if used.is_empty() {
        format!("Фрагментів бібліотеки за цим запитанням не знайдено.\n\nЗапитання: {question}")
    } else {
        format!("Фрагменти бібліотеки:\n\n{ctx}Запитання: {question}")
    };
    RagPrompt {
        system: SYSTEM_PROMPT.to_string(),
        user,
        passages: used,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Card, Category};

    #[test]
    fn prompt_cites_sources() {
        let lib = Library::new(vec![Card::simple(
            "bleed",
            Category::FirstAid,
            "Сильна кровотеча",
            &["Притисніть рану", "Накладіть турнікет"],
            "Наказ МОЗ №441",
        )]);
        let p = build_prompt(&lib, "як зупинити кровотечу", 4, 4000);
        assert_eq!(p.passages.len(), 1);
        assert!(p.user.contains("[1] Сильна кровотеча"));
        assert!(p.user.contains("не перевірено"));
        let p = build_prompt(&lib, "космос", 4, 4000);
        assert!(p.passages.is_empty());
        assert!(p.user.contains("не знайдено"));
    }
}
