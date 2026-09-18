use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Default constituent statuses for the [`FilterReadStatus::Active`] alias.
///
/// Expanded at query time — not stored as individual statuses.
/// Future: per-user configuration may override which statuses constitute Active
/// (e.g. a user who wants Paused included).
pub const ACTIVE_STATUSES: &[FilterReadStatus] = &[FilterReadStatus::Unread, FilterReadStatus::Reading, FilterReadStatus::Rereading];

/// A composable, recursive filter over the book catalog.
///
/// Can be applied to smart shelves, the search bar, or composed via
/// [`BookFilter::and`] / [`BookFilter::or`] (e.g.
/// `shelf_filter.and(search_filter)`). Stored as JSONB in
/// `shelves.filter_criteria`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum BookFilter {
    Group(FilterGroup),
    Rule(FilterRule),
}

impl BookFilter {
    /// Combine two filters with AND semantics.
    #[must_use]
    pub fn and(self, other: Self) -> Self {
        Self::Group(FilterGroup {
            condition: FilterCondition::And,
            items: vec![self, other],
        })
    }

    /// Combine two filters with OR semantics.
    #[must_use]
    pub fn or(self, other: Self) -> Self {
        Self::Group(FilterGroup {
            condition: FilterCondition::Or,
            items: vec![self, other],
        })
    }

    /// Returns `true` if this filter (or any nested sub-filter) contains a
    /// [`FilterRule::Library`] rule.
    ///
    /// When `true`, the caller should skip applying the active-library scope
    /// on book queries, because the filter already handles library scoping.
    pub fn contains_library_rule(&self) -> bool {
        match self {
            Self::Rule(FilterRule::Library { .. }) => true,
            Self::Rule(_) => false,
            Self::Group(group) => group.items.iter().any(Self::contains_library_rule),
        }
    }

    /// Returns `true` if this filter (or any nested sub-filter) contains a
    /// rule that requires a specific user context to evaluate (e.g.
    /// `ReadStatus`).
    ///
    /// Use this to guard APIs that apply a filter without a user identity.
    pub fn contains_user_scoped_rules(&self) -> bool {
        match self {
            Self::Rule(FilterRule::ReadStatus { .. }) => true,
            Self::Rule(_) => false,
            Self::Group(group) => group.items.iter().any(Self::contains_user_scoped_rules),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilterGroup {
    pub condition: FilterCondition,
    pub items: Vec<BookFilter>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterCondition {
    And,
    Or,
}

/// A resolved entity reference for pill-picker rule variants.
///
/// `label` is stored alongside `id` for UI rendering — not used in DB queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityRef {
    pub id: i64,
    pub label: String,
}

// --- Operator enums ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextOp {
    Contains,
    DoesntContain,
    StartsWith,
    EndsWith,
    Equals,
    NotEquals,
    IsEmpty,
    IsNotEmpty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetOp {
    IncludesAny,
    IncludesAll,
    ExcludesAll,
    IsEmpty,
    IsNotEmpty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumericOp {
    Eq,
    NotEq,
    Lt,
    Lte,
    Gt,
    Gte,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateOp {
    Before,
    After,
    IsEmpty,
    IsNotEmpty,
}

/// Read status values usable in filter rules.
///
/// Mirrors [`crate::reading::ReadStatus`] but adds the
/// [`Active`](FilterReadStatus::Active) alias, which expands at query time to
/// the statuses listed in [`ACTIVE_STATUSES`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterReadStatus {
    Unread,
    Reading,
    Paused,
    Rereading,
    Read,
    Abandoned,
    /// Convenience alias for books to sync to a device.
    /// Expands to [`ACTIVE_STATUSES`] at query time.
    /// Future: per-user configuration may override the constituent statuses.
    Active,
}

/// A single filter condition on a specific book field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "field", content = "params", rename_all = "snake_case")]
pub enum FilterRule {
    // Text search — used by the search bar
    TitleText {
        op: TextOp,
        value: String,
    },
    AuthorText {
        op: TextOp,
        value: String,
    },

    // Entity pill-picker — always resolved to EntityRef before storage
    Author {
        op: SetOp,
        values: Vec<EntityRef>,
    },
    Series {
        op: SetOp,
        values: Vec<EntityRef>,
    },
    Genre {
        op: SetOp,
        values: Vec<EntityRef>,
    },
    Tag {
        op: SetOp,
        values: Vec<EntityRef>,
    },
    Publisher {
        op: SetOp,
        values: Vec<EntityRef>,
    },
    Shelf {
        op: SetOp,
        values: Vec<EntityRef>,
    },
    /// Admin-only: filter by library membership.
    /// When present, bypasses the active-library scope on book queries.
    Library {
        op: SetOp,
        values: Vec<EntityRef>,
    },

    // Other fields
    Language {
        op: SetOp,
        values: Vec<String>,
    },
    ReadStatus {
        op: SetOp,
        values: Vec<FilterReadStatus>,
    },
    Rating {
        op: NumericOp,
        value: u8,
    },
    /// `value` is `None` for `IsEmpty` / `IsNotEmpty` operators.
    DateAdded {
        op: DateOp,
        value: Option<DateTime<Utc>>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn title_contains(s: &str) -> BookFilter {
        BookFilter::Rule(FilterRule::TitleText {
            op: TextOp::Contains,
            value: s.to_string(),
        })
    }

    fn entity(id: i64, label: &str) -> EntityRef {
        EntityRef { id, label: label.to_string() }
    }

    #[test]
    fn active_status_round_trips_as_active() {
        let json = serde_json::to_string(&FilterReadStatus::Active).unwrap();
        assert_eq!(json, "\"active\"");
        let back: FilterReadStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, FilterReadStatus::Active);
    }

    #[test]
    fn active_statuses_constant_contents() {
        assert!(ACTIVE_STATUSES.contains(&FilterReadStatus::Unread));
        assert!(ACTIVE_STATUSES.contains(&FilterReadStatus::Reading));
        assert!(ACTIVE_STATUSES.contains(&FilterReadStatus::Rereading));
        assert!(!ACTIVE_STATUSES.contains(&FilterReadStatus::Paused));
        assert!(!ACTIVE_STATUSES.contains(&FilterReadStatus::Read));
        assert!(!ACTIVE_STATUSES.contains(&FilterReadStatus::Abandoned));
        assert!(!ACTIVE_STATUSES.contains(&FilterReadStatus::Active));
    }

    #[test]
    fn and_composition_produces_and_group() {
        let a = title_contains("foo");
        let b = title_contains("bar");
        assert_eq!(
            a.clone().and(b.clone()),
            BookFilter::Group(FilterGroup {
                condition: FilterCondition::And,
                items: vec![a, b],
            })
        );
    }

    #[test]
    fn or_composition_produces_or_group() {
        let a = title_contains("foo");
        let b = title_contains("bar");
        assert_eq!(
            a.clone().or(b.clone()),
            BookFilter::Group(FilterGroup {
                condition: FilterCondition::Or,
                items: vec![a, b],
            })
        );
    }

    #[test]
    fn composite_filter_round_trip() {
        // AND { Author IncludesAny [Tata, Thor], ReadStatus IncludesAny
        // [Active] }
        let filter = BookFilter::Rule(FilterRule::Author {
            op: SetOp::IncludesAny,
            values: vec![entity(42, "A. J. Tata"), entity(17, "Brad Thor")],
        })
        .and(BookFilter::Rule(FilterRule::ReadStatus {
            op: SetOp::IncludesAny,
            values: vec![FilterReadStatus::Active],
        }));

        let json = serde_json::to_string(&filter).unwrap();
        let back: BookFilter = serde_json::from_str(&json).unwrap();
        assert_eq!(filter, back);
    }

    #[test]
    fn date_added_with_value_round_trip() {
        let filter = BookFilter::Rule(FilterRule::DateAdded {
            op: DateOp::After,
            value: Some(Utc::now()),
        });
        let json = serde_json::to_string(&filter).unwrap();
        let back: BookFilter = serde_json::from_str(&json).unwrap();
        assert_eq!(filter, back);
    }

    #[test]
    fn date_added_is_empty_round_trip() {
        let filter = BookFilter::Rule(FilterRule::DateAdded {
            op: DateOp::IsEmpty,
            value: None,
        });
        let json = serde_json::to_string(&filter).unwrap();
        let back: BookFilter = serde_json::from_str(&json).unwrap();
        assert_eq!(filter, back);
    }

    #[test]
    fn all_filter_rule_variants_round_trip() {
        let rules: Vec<FilterRule> = vec![
            FilterRule::TitleText {
                op: TextOp::Contains,
                value: "Dune".to_string(),
            },
            FilterRule::AuthorText {
                op: TextOp::StartsWith,
                value: "Her".to_string(),
            },
            FilterRule::Author {
                op: SetOp::IncludesAny,
                values: vec![entity(1, "Frank Herbert")],
            },
            FilterRule::Series {
                op: SetOp::IncludesAll,
                values: vec![entity(2, "Dune")],
            },
            FilterRule::Genre {
                op: SetOp::ExcludesAll,
                values: vec![],
            },
            FilterRule::Tag {
                op: SetOp::IsEmpty,
                values: vec![],
            },
            FilterRule::Publisher {
                op: SetOp::IsNotEmpty,
                values: vec![],
            },
            FilterRule::Shelf {
                op: SetOp::IncludesAny,
                values: vec![entity(10, "Fantasy Reads")],
            },
            FilterRule::Library {
                op: SetOp::ExcludesAll,
                values: vec![entity(1, "Scotte's Library")],
            },
            FilterRule::Language {
                op: SetOp::IncludesAny,
                values: vec!["en".to_string()],
            },
            FilterRule::ReadStatus {
                op: SetOp::IncludesAny,
                values: vec![FilterReadStatus::Active],
            },
            FilterRule::Rating { op: NumericOp::Gte, value: 4 },
            FilterRule::DateAdded {
                op: DateOp::After,
                value: Some(Utc::now()),
            },
        ];

        for rule in rules {
            let filter = BookFilter::Rule(rule);
            let json = serde_json::to_string(&filter).unwrap();
            let back: BookFilter = serde_json::from_str(&json).unwrap();
            assert_eq!(filter, back);
        }
    }
}
