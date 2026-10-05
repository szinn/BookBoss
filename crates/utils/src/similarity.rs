/// Weight applied to the title similarity component of the combined score.
pub const TITLE_WEIGHT: f32 = 0.7;

/// Weight applied to the author similarity component of the combined score.
pub const AUTHOR_WEIGHT: f32 = 0.3;

/// Combine a title and author similarity score into a single value using
/// [`TITLE_WEIGHT`] and [`AUTHOR_WEIGHT`].
///
/// If no author is available for comparison, pass `0.5` as `author_score`
/// to apply a neutral contribution.
#[must_use]
pub fn combined_score(title_score: f32, author_score: f32) -> f32 {
    TITLE_WEIGHT * title_score + AUTHOR_WEIGHT * author_score
}

/// Maximum number of tokens in a leading segment for it to be treated as an
/// episode/series marker (e.g. `Ep.#3.9`, `The Expanse #3`).
const MAX_PREFIX_TOKENS: usize = 3;

/// Minimum number of normalized words the text after an episode/series marker
/// must have to be used as a comparison variant.
const MIN_REMAINDER_WORDS: usize = 2;

/// Compute word-level Jaccard similarity between two title strings.
///
/// Each title is normalized by removing punctuation, lowercasing, and dropping
/// leading articles ("the", "a", "an"). Two variants are compared for each
/// side and the best pairwise score is returned:
///
/// - the main title, with any subtitle (text after ` : ` or ` - `) removed;
/// - the text after the first separator, when the text before it is a short
///   episode/series marker containing a digit or `#` (e.g. `Ep.#3.9 - Title`).
///
/// Returns a value in `[0.0, 1.0]`; `1.0` means identical word sets.
#[must_use]
pub fn title_similarity(a: &str, b: &str) -> f32 {
    let a_variants = title_variants(a);
    let b_variants = title_variants(b);
    a_variants
        .iter()
        .flat_map(|x| b_variants.iter().map(move |y| word_jaccard(x, y)))
        .fold(0.0, f32::max)
}

/// Compute word-level Jaccard similarity between two author name strings.
///
/// Normalizes each name by removing punctuation and lowercasing before
/// comparing word sets. Order-independent, so "Smith, Jane" and "Jane Smith"
/// produce the same score.
///
/// Returns a value in `[0.0, 1.0]`; `1.0` means identical word sets.
#[must_use]
pub fn author_similarity(a: &str, b: &str) -> f32 {
    word_jaccard(&normalize_author(a), &normalize_author(b))
}

fn title_variants(s: &str) -> Vec<String> {
    let mut variants = vec![normalize_words(main_title(s))];
    if let Some(rest) = strip_episode_prefix(s) {
        let rest = normalize_words(rest);
        if rest.split_whitespace().count() >= MIN_REMAINDER_WORDS {
            variants.push(rest);
        }
    }
    variants
}

fn main_title(s: &str) -> &str {
    s.split(" : ").next().and_then(|t| t.split(" - ").next()).unwrap_or(s)
}

/// Returns the text after the first ` - ` / ` : ` separator when the text
/// before it looks like an episode/series marker.
fn strip_episode_prefix(s: &str) -> Option<&str> {
    let (prefix, rest) = [" - ", " : "]
        .iter()
        .filter_map(|sep| s.split_once(*sep))
        .min_by_key(|(prefix, _)| prefix.len())?;
    let short = prefix.split_whitespace().count() <= MAX_PREFIX_TOKENS;
    let numbered = prefix.chars().any(|c| c.is_ascii_digit() || c == '#');
    (short && numbered).then_some(rest)
}

fn normalize_words(s: &str) -> String {
    let cleaned: String = s.to_lowercase().chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
    cleaned
        .split_whitespace()
        .filter(|w| !matches!(*w, "the" | "a" | "an"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_author(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn word_jaccard(a: &str, b: &str) -> f32 {
    let a_words: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let b_words: std::collections::HashSet<&str> = b.split_whitespace().collect();
    let union_count = a_words.union(&b_words).count();
    if union_count == 0 {
        return 1.0;
    }
    #[expect(clippy::cast_precision_loss, reason = "word counts are small; f32 precision is sufficient")]
    {
        a_words.intersection(&b_words).count() as f32 / union_count as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_titles_score_one() {
        assert!((title_similarity("The Hobbit", "The Hobbit") - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn article_stripping() {
        assert!((title_similarity("The Hobbit", "Hobbit") - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn subtitle_stripping() {
        let score = title_similarity(
            "Fellowship of the Ring",
            "The Fellowship of the Ring : Being the First Part of The Lord of the Rings",
        );
        assert!(score > 0.9, "score was {score}");
    }

    #[test]
    fn unrelated_titles_score_low() {
        let score = title_similarity("The Hobbit", "Pride and Prejudice");
        assert!(score < 0.2, "score was {score}");
    }

    #[test]
    fn author_order_independent() {
        assert!((author_similarity("Smith, Jane", "Jane Smith") - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn combined_score_weights() {
        let s = combined_score(1.0, 0.0);
        assert!((s - TITLE_WEIGHT).abs() < f32::EPSILON);
    }

    #[test]
    fn episode_prefix_regression_gh_322() {
        let wanted = "Ep.#3.9 - \"Lesser of Two Evils\"";
        let unrelated = "The Lesser of Two Evils";
        let input = "Lesser of Two Evils";

        let title_score = title_similarity(input, wanted);
        assert!((title_score - 1.0).abs() < f32::EPSILON, "score was {title_score}");

        let brown = combined_score(title_score, author_similarity("Ryk Brown", "Ryk Brown"));
        let pilkington = combined_score(title_similarity(input, unrelated), author_similarity("Ryk Brown", "Amy Dodd Pilkington"));
        assert!(brown > pilkington, "brown {brown} <= pilkington {pilkington}");
    }

    #[test]
    fn episode_prefix_symmetric() {
        let wanted = "Ep.#3.9 - \"Lesser of Two Evils\"";
        let input = "Lesser of Two Evils";
        let swapped = title_similarity(wanted, input);
        assert!((swapped - 1.0).abs() < f32::EPSILON, "score was {swapped}");
        assert!((swapped - title_similarity(input, wanted)).abs() < f32::EPSILON);
    }

    #[test]
    fn series_hash_prefix_stripped() {
        let score = title_similarity("The Expanse #3 - Abaddon's Gate", "Abaddon's Gate");
        assert!((score - 1.0).abs() < f32::EPSILON, "score was {score}");
    }

    #[test]
    fn colon_separator_prefix_stripped() {
        let score = title_similarity("Book 2 : The Shadow Rising", "The Shadow Rising");
        assert!((score - 1.0).abs() < f32::EPSILON, "score was {score}");
    }

    #[test]
    fn non_numeric_prefix_not_stripped() {
        let score = title_similarity("Dune - A Novel", "A Novel");
        assert!(score < 0.2, "score was {score}");
    }

    #[test]
    fn numeric_title_with_one_word_subtitle_not_stripped() {
        let score = title_similarity("1984 - A Novel", "A Novel");
        assert!(score < 0.2, "score was {score}");
    }

    #[test]
    fn long_numbered_prefix_not_stripped() {
        let score = title_similarity("Book 2 of the Long Saga - Something Else", "Something Else");
        assert!(score < 0.2, "score was {score}");
    }
}
