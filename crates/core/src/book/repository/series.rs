use crate::{
    Error,
    book::{NewSeries, Series, SeriesId, SeriesToken},
    repository::Transaction,
};

#[async_trait::async_trait]
pub trait SeriesRepository: Send + Sync {
    async fn add_series(&self, transaction: &dyn Transaction, series: NewSeries) -> Result<Series, Error>;
    async fn update_series(&self, transaction: &dyn Transaction, series: Series) -> Result<Series, Error>;
    async fn find_by_id(&self, transaction: &dyn Transaction, id: SeriesId) -> Result<Option<Series>, Error>;
    async fn find_by_token(&self, transaction: &dyn Transaction, token: &SeriesToken) -> Result<Option<Series>, Error>;
    async fn list_series(&self, transaction: &dyn Transaction, start_id: Option<SeriesId>, page_size: Option<u64>) -> Result<Vec<Series>, Error>;
    async fn find_by_name(&self, transaction: &dyn Transaction, name: &str) -> Result<Option<Series>, Error>;
}
