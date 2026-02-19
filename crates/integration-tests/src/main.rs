mod context;

#[cfg(feature = "postgres")]
mod postgres;

#[cfg(feature = "sqlite")]
#[cfg_attr(feature = "postgres", allow(dead_code))]
mod sqlite;

#[cfg(feature = "mysql")]
#[cfg_attr(feature = "postgres", allow(dead_code))]
mod mysql;

// Priority: postgres > mysql > sqlite when multiple features are enabled.
#[cfg(all(feature = "mysql", not(feature = "postgres")))]
use mysql::setup;
#[cfg(feature = "postgres")]
use postgres::setup;
#[cfg(all(feature = "sqlite", not(feature = "postgres"), not(feature = "mysql")))]
use sqlite::setup;

#[cfg(not(any(feature = "postgres", feature = "sqlite", feature = "mysql")))]
compile_error!("At least one database backend feature must be enabled: postgres, sqlite, or mysql");

#[tokio::test]
async fn test_setup() {
    let _ctx = setup().await;
}
