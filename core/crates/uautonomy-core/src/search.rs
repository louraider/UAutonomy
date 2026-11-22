use crate::card::Card;

/// Найпростіший офлайн-пошук по картках: збіг усіх слів запиту без
/// урахування регістру. Пізніше його замінить повнотекстовий індекс
/// (tantivy або вбудований у ZIM) з морфологією української.
#[derive(Debug, Default)]
pub struct Library {
    cards: Vec<Card>,
}

impl Library {
    pub fn new(cards: Vec<Card>) -> Self {
        Self { cards }
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    pub fn search(&self, query: &str) -> Vec<&Card> {
        let words: Vec<String> = query.split_whitespace().map(|w| w.to_lowercase()).collect();
        if words.is_empty() {
            return Vec::new();
        }
        self.cards
            .iter()
            .filter(|c| {
                let hay = format!("{} {}", c.title, c.steps.join(" ")).to_lowercase();
                words.iter().all(|w| hay.contains(w.as_str()))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::Category;

    fn card(title: &str, step: &str) -> Card {
        Card {
            id: title.into(),
            category: Category::FirstAid,
            title: title.into(),
            steps: vec![step.into()],
            source: "тест".into(),
            reviewed: false,
        }
    }

    #[test]
    fn finds_ukrainian_case_insensitive() {
        let lib = Library::new(vec![
            card("Сильна кровотеча", "Накладіть турнікет"),
            card("Опік", "Охолодіть водою"),
        ]);
        let hits = lib.search("ТУРНІКЕТ");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Сильна кровотеча");
        assert!(lib.search("   ").is_empty());
    }
}
