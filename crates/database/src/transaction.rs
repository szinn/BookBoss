use std::any::Any;

use bb_core::{Error, repository::Transaction};
use sea_orm::DatabaseTransaction;

use crate::error::handle_dberr;

pub(crate) struct TransactionImpl {
    pub(crate) transaction: DatabaseTransaction,
}

impl<'a> TransactionImpl {
    pub(crate) fn new(transaction: DatabaseTransaction) -> Self {
        Self { transaction }
    }

    pub(crate) fn get_db_transaction(tx: &'a dyn Transaction) -> Result<&'a DatabaseTransaction, Error> {
        match tx.as_any().downcast_ref::<TransactionImpl>() {
            Some(transaction) => Ok(&transaction.transaction),
            _ => Err(Error::InvalidTransactionType),
        }
    }
}

#[async_trait::async_trait]
impl Transaction for TransactionImpl {
    fn as_any(&self) -> &dyn Any {
        self
    }

    async fn commit(self: Box<Self>) -> Result<(), Error> {
        self.transaction.commit().await.map_err(handle_dberr)?;
        Ok(())
    }

    async fn rollback(self: Box<Self>) -> Result<(), Error> {
        self.transaction.rollback().await.map_err(handle_dberr)?;
        Ok(())
    }
}
