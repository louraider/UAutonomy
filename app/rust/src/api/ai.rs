use uautonomy_core::rag;

use crate::api::library::CardView;
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

/// Як відповідати на запитання: `generate` — модель відповідає з опорою на
/// бібліотеку; `cards` — модель мовчить, застосунок показує `message` і
/// готові картки (медичні запитання, міни, кризові повідомлення).
pub struct RouteView {
    pub mode: String,
    /// `medical`, `mines` або `crisis` для режиму `cards`.
    pub topic: Option<String>,
    pub message: String,
    pub cards: Vec<CardView>,
}

#[flutter_rust_bridge::frb(sync)]
pub fn ask_route(question: String) -> RouteView {
    let generate = RouteView {
        mode: "generate".into(),
        topic: None,
        message: String::new(),
        cards: Vec::new(),
    };
    let Ok(a) = state::app_ref() else {
        return generate;
    };
    let s = a.shared.read().unwrap();
    match rag::route(&s.library, &question) {
        rag::Route::Generate => generate,
        rag::Route::Cards {
            topic,
            message,
            card_ids,
        } => RouteView {
            mode: "cards".into(),
            topic: Some(
                match topic {
                    rag::Topic::Medical => "medical",
                    rag::Topic::Mines => "mines",
                    rag::Topic::Crisis => "crisis",
                }
                .into(),
            ),
            message,
            cards: card_ids
                .iter()
                .filter_map(|id| s.library.card(id))
                .map(CardView::from)
                .collect(),
        },
    }
}
