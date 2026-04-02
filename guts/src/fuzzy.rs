use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

/// Result of a fuzzy search match containing the index and score
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    pub index: usize,
    pub score: i64,
}

/// Performs fuzzy search on a list of strings and returns matches sorted by score
pub fn fuzzy_search(items: &[String], pattern: &str) -> Vec<FuzzyMatch> {
    if pattern.is_empty() {
        return items
            .iter()
            .enumerate()
            .map(|(idx, _)| FuzzyMatch {
                index: idx,
                score: 0,
            })
            .collect();
    }

    let matcher = SkimMatcherV2::default();
    let mut matches: Vec<FuzzyMatch> = items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            matcher
                .fuzzy_match(item, pattern)
                .map(|score| FuzzyMatch { index: idx, score })
        })
        .collect();

    // Sort by score descending (higher score = better match)
    matches.sort_by(|a, b| b.score.cmp(&a.score));
    matches
}

/// Performs fuzzy search on column headers
pub fn fuzzy_search_columns(headers: &[String], pattern: &str) -> Vec<FuzzyMatch> {
    fuzzy_search(headers, pattern)
}

/// Performs fuzzy search on table names
#[allow(dead_code)]
pub fn fuzzy_search_tables(tables: &[String], pattern: &str) -> Vec<FuzzyMatch> {
    fuzzy_search(tables, pattern)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_search_basic() {
        let items = vec![
            "apple".to_string(),
            "application".to_string(),
            "apply".to_string(),
            "banana".to_string(),
        ];
        let matches = fuzzy_search(&items, "app");

        assert!(!matches.is_empty());
        // All app* items should match and be sorted by score
        assert_eq!(matches.len(), 3);
        let matched_indices: Vec<usize> = matches.iter().map(|m| m.index).collect();
        assert!(matched_indices.contains(&0));
        assert!(matched_indices.contains(&1));
        assert!(matched_indices.contains(&2));
        assert!(!matched_indices.contains(&3));
        assert!(matches.windows(2).all(|w| w[0].score >= w[1].score));
    }

    #[test]
    fn test_fuzzy_search_empty_pattern() {
        let items = vec!["one".to_string(), "two".to_string(), "three".to_string()];
        let matches = fuzzy_search(&items, "");

        // Empty pattern should return all items
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_fuzzy_search_columns() {
        let headers = vec![
            "id".to_string(),
            "name".to_string(),
            "email".to_string(),
            "created_at".to_string(),
        ];
        let matches = fuzzy_search_columns(&headers, "ema");

        assert!(!matches.is_empty());
        // "email" should match
        assert_eq!(matches[0].index, 2);
    }

    #[test]
    fn test_fuzzy_search_no_match() {
        let items = vec!["apple".to_string(), "banana".to_string()];
        let matches = fuzzy_search(&items, "xyz");

        // No matches should return empty vector
        assert!(matches.is_empty());
    }
}
