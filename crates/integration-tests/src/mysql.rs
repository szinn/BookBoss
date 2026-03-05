use bb_database::create_repository_service;
use sea_orm::Database;
use testcontainers::{ImageExt as _, runners::AsyncRunner as _};
use testcontainers_modules::mysql::Mysql;

use crate::context::TestContext;

pub async fn setup() -> TestContext {
    let container = Mysql::default().with_tag("8").start().await.unwrap();
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(3306).await.unwrap();
    let url = format!("mysql://root@{host}:{port}/mysql");

    let db = Database::connect(&url).await.unwrap();
    let repository_service = create_repository_service(db).await.unwrap();
    let core_services = bb_core::create_services(
        repository_service,
        bb_core::test_support::nop_library_store(),
        bb_core::test_support::nop_pipeline_service(),
    )
    .unwrap();

    TestContext::new(core_services, container)
}
