use uautonomy_core::rag;

use crate::state;

pub struct SourceView {
    pub number: u32,
    pub key: String,
    pub title: String,
    pub source: String,
    pub reviewed: bool,
}

pub struct RagView {
    pub system: String,
    pub user: String,
    pub sources: Vec<SourceView>,
}

/// Готує запит до локальної моделі: інструкції, фрагменти бібліотеки
/// з номерами й список джерел для показу під відповіддю.
/// `context_tokens` — розмір контексту моделі.
pub fn build_rag_prompt(question: String, context_tokens: u32) -> RagView {
    let Ok(a) = state::app_ref() else {
        return RagView {
            system: rag::SYSTEM_PROMPT.into(),
            user: question,
            sources: Vec::new(),
        };
    };
    // Половина контексту під фрагменти, ~3 символи на токен.
    let max_chars = (context_tokens as usize / 2) * 3;
    let s = a.shared.read().unwrap();
    let p = rag::build_prompt(&s.library, &question, 5, max_chars);
    RagView {
        system: p.system,
        user: p.user,
        sources: p
            .passages
            .into_iter()
            .enumerate()
            .map(|(i, x)| SourceView {
                number: i as u32 + 1,
                key: x.key,
                title: x.title,
                source: x.source,
                reviewed: x.reviewed,
            })
            .collect(),
    }
}
