use crate::account_service::AccountServiceError;
use crate::tx::*;

use std::collections::HashMap;

#[derive(Debug, PartialEq)]
pub enum TransactionState {
    Valid,
    Disputed,
    Refunded,
    // Rejected, // we store only disputable transactions in this impl
}

pub struct TransactionWithState {
    pub tx: Transaction,
    pub state: TransactionState,
}

type TransactionStorage = HashMap<TransactionId, TransactionWithState>;

pub struct TransactionService {
    // TODO: remove pub (provide access method to vacant entry)
    pub trans: TransactionStorage,
}

impl TransactionService {
    pub fn new() -> Self {
        Self {
            trans: TransactionStorage::default(),
        }
    }

    pub fn get_mut(
        &mut self,
        transaction_id: TransactionId,
    ) -> Result<&mut TransactionWithState, AccountServiceError> {
        self.trans
            .get_mut(&transaction_id)
            .ok_or(AccountServiceError::TransactionNotFound)
    }
}
