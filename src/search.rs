use crate::history::History;
use crate::model::Entry;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

pub struct SearchMatch {
    pub index: usize,
    pub score: i64,
    pub indices: Vec<usize>,
}

pub fn filtered_matches(entries: &[Entry], query: &str, history: &History) -> Vec<SearchMatch> {
    let matcher = SkimMatcherV2::default();

    let mut matches: Vec<SearchMatch> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let history_score = history.score(&entry.history_key());

            if query.is_empty() {
                return Some(SearchMatch {
                    index,
                    score: history_score,
                    indices: Vec::new(),
                });
            }

            matcher
                .fuzzy_indices(&entry.search_text(), query)
                .map(|(score, indices)| SearchMatch {
                    index,
                    score: score + score.saturating_mul(history_score) / 100,
                    indices: display_match_indices(&matcher, entry, query).unwrap_or(indices),
                })
        })
        .collect();

    matches.sort_by(|left, right| {
        let left_entry = &entries[left.index];
        let right_entry = &entries[right.index];

        right
            .score
            .cmp(&left.score)
            .then_with(|| {
                right_entry
                    .is_active_tmux()
                    .cmp(&left_entry.is_active_tmux())
            })
            .then_with(|| right_entry.type_rank().cmp(&left_entry.type_rank()))
            .then_with(|| left.index.cmp(&right.index))
    });

    matches
}

fn display_match_indices(
    matcher: &SkimMatcherV2,
    entry: &Entry,
    query: &str,
) -> Option<Vec<usize>> {
    matcher
        .fuzzy_indices(&entry.display_search_text(), query)
        .map(|(_, indices)| indices)
}
