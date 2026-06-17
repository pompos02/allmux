use crate::history::History;
use crate::model::Entry;
use neo_frizbee::Scoring;

pub struct SearchMatch {
    pub index: usize,
    pub score: i64,
    pub indices: Vec<usize>,
}

pub fn filtered_matches(entries: &[Entry], query: &str, history: &History) -> Vec<SearchMatch> {
    let query_parts: Vec<&str> = query.split_whitespace().collect();

    let mut matches: Vec<SearchMatch> = if query_parts.is_empty() {
        entries
            .iter()
            .enumerate()
            .map(|(index, entry)| SearchMatch {
                index,
                score: history.score(&entry.history_key()),
                indices: Vec::new(),
            })
            .collect()
    } else {
        entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| match_entry(entry, index, &query_parts, history))
            .collect()
    };

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

fn match_entry(
    entry: &Entry,
    index: usize,
    query_parts: &[&str],
    history: &History,
) -> Option<SearchMatch> {
    let search_text = entry.search_text();
    let display_text = entry.display_search_text();
    let mut score_total: i64 = 0;
    let mut indices = Vec::new();

    for part in query_parts {
        let options = matcher_config(part);
        let mut matcher = neo_frizbee::Matcher::new(part, &options);
        let part_match = matcher.match_list_indices(&[search_text.as_str()]).pop()?;

        score_total = score_total.saturating_add(i64::from(part_match.score));

        if let Some(display_match) = matcher.match_list_indices(&[display_text.as_str()]).pop() {
            indices.extend(byte_indices_to_char_indices(
                &display_text,
                display_match.indices,
            ));
        } else {
            indices.extend(byte_indices_to_char_indices(
                &search_text,
                part_match.indices,
            ));
        }
    }

    indices.sort_unstable();
    indices.dedup();

    let base_score = score_total / query_parts.len() as i64;
    let history_score = history.score(&entry.history_key());

    Some(SearchMatch {
        index,
        score: base_score.saturating_add(base_score.saturating_mul(history_score) / 100),
        indices,
    })
}

fn matcher_config(query: &str) -> neo_frizbee::Config {
    let has_uppercase = query.chars().any(char::is_uppercase);

    neo_frizbee::Config {
        max_typos: Some(
            (query.len() as u16 / 4)
                .clamp(2, 6)
                .min((query.len() as u16).saturating_sub(1)),
        ),
        sort: false,
        scoring: Scoring {
            capitalization_bonus: if has_uppercase { 8 } else { 0 },
            matching_case_bonus: if has_uppercase { 4 } else { 0 },
            ..Default::default()
        },
    }
}

fn byte_indices_to_char_indices(text: &str, indices: Vec<usize>) -> Vec<usize> {
    indices
        .into_iter()
        .filter(|&index| index < text.len() && text.is_char_boundary(index))
        .map(|index| text[..index].chars().count())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DockerContainer, Entry, SshHost};

    #[test]
    fn single_character_query_does_not_match_absent_character() {
        let entries = [ssh_entry("alpha", "example.local")];

        let matches = filtered_matches(&entries, "z", &History::default());

        assert!(matches.is_empty());
    }

    #[test]
    fn multi_word_query_matches_terms_in_any_field_order() {
        let entries = [docker_entry("nginx", true)];

        let matches = filtered_matches(&entries, "running nginx", &History::default());

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].index, 0);
    }

    #[test]
    fn match_indices_are_sorted_character_offsets() {
        let entries = [ssh_entry("régulière", "example.local")];

        let matches = filtered_matches(&entries, "guli", &History::default());

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].indices, vec![2, 3, 4, 5]);
    }

    fn ssh_entry(alias: &str, hostname: &str) -> Entry {
        Entry::Ssh(SshHost {
            alias: alias.to_string(),
            hostname: hostname.to_string(),
            user: String::new(),
            description: None,
            is_active_tmux: false,
        })
    }

    fn docker_entry(name: &str, running: bool) -> Entry {
        Entry::Docker(DockerContainer {
            id: String::new(),
            name: name.to_string(),
            image: String::new(),
            command: String::new(),
            created_at: String::new(),
            status_text: String::new(),
            ports: String::new(),
            status: running,
            is_active_tmux: false,
        })
    }
}
