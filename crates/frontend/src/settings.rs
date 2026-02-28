use serde::{Deserialize, Serialize};

/// The available views for the book library display.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub(crate) enum BookDisplayView {
    #[default]
    IconView,
    TableView,
}

impl std::fmt::Display for BookDisplayView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IconView => write!(f, "icon_view"),
            Self::TableView => write!(f, "table_view"),
        }
    }
}

impl std::str::FromStr for BookDisplayView {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "icon_view" => Ok(Self::IconView),
            "table_view" => Ok(Self::TableView),
            _ => Err(()),
        }
    }
}

/// Frontend-specific setting keys for the user setting store.
///
/// Keys use the `frontend:` namespace to avoid collisions with other adapters.
#[cfg(feature = "server")]
pub(crate) enum FrontendSettings {
    ApiKey,
    BookDisplayView,
}

#[cfg(feature = "server")]
impl FrontendSettings {
    pub(crate) fn key(&self) -> &'static str {
        match self {
            Self::ApiKey => "frontend:api_key",
            Self::BookDisplayView => "frontend:book_display_view",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn book_display_view_default_is_icon_view() {
        assert_eq!(BookDisplayView::default(), BookDisplayView::IconView);
    }

    #[test]
    fn book_display_view_display() {
        assert_eq!(BookDisplayView::IconView.to_string(), "icon_view");
        assert_eq!(BookDisplayView::TableView.to_string(), "table_view");
    }

    #[test]
    fn book_display_view_from_str() {
        assert_eq!("icon_view".parse::<BookDisplayView>(), Ok(BookDisplayView::IconView));
        assert_eq!("table_view".parse::<BookDisplayView>(), Ok(BookDisplayView::TableView));
        assert_eq!("unknown".parse::<BookDisplayView>(), Err(()));
        assert_eq!("".parse::<BookDisplayView>(), Err(()));
    }

    #[test]
    fn book_display_view_round_trip() {
        for variant in [BookDisplayView::IconView, BookDisplayView::TableView] {
            let serialized = variant.to_string();
            let parsed: BookDisplayView = serialized.parse().expect("round-trip parse failed");
            assert_eq!(parsed, variant);
        }
    }

    #[cfg(feature = "server")]
    mod server {
        use super::super::FrontendSettings;

        #[test]
        fn frontend_settings_keys_are_namespaced() {
            assert_eq!(FrontendSettings::ApiKey.key(), "frontend:api_key");
            assert_eq!(FrontendSettings::BookDisplayView.key(), "frontend:book_display_view");
        }

        #[test]
        fn frontend_settings_keys_are_unique() {
            let keys = [FrontendSettings::ApiKey.key(), FrontendSettings::BookDisplayView.key()];
            let unique: std::collections::HashSet<_> = keys.iter().collect();
            assert_eq!(unique.len(), keys.len());
        }
    }
}
