use crate::account_service::{Account, AccountService, AccountServiceError};
use crate::tx::*;
use crate::tx_service::{TransactionService, TransactionState, TransactionWithState};

use std::collections::hash_map::Entry;

pub struct TransactionProcessor {}

// business logic for transaction processing
impl TransactionProcessor {
    pub fn process(
        account_service: &mut AccountService,
        tx_service: &mut TransactionService,
        tx: Transaction,
    ) -> Result<(), AccountServiceError> {
        let account = account_service.ensure_account(tx.client_id);
        if account.locked {
            return Err(AccountServiceError::AccountLocked);
        }
        match tx.tx_type {
            TransactionType::Deposit => TransactionProcessor::deposit(account, tx_service, tx),
            TransactionType::Withdrawal => {
                TransactionProcessor::withdrawal(account, tx_service, tx)
            }
            TransactionType::Dispute => TransactionProcessor::dispute(account, tx_service, tx),
            TransactionType::Resolve => TransactionProcessor::resolve(account, tx_service, tx),
            TransactionType::Chargeback => {
                TransactionProcessor::chargeback(account, tx_service, tx)
            }
        }
    }

    fn deposit(
        account: &mut Account,
        tx_service: &mut TransactionService,
        tx: Transaction,
    ) -> Result<(), AccountServiceError> {
        let amount = match tx.amount {
            Some(v) => v,
            None => return Err(AccountServiceError::EmptyTransactionAmount),
        };

        let trans_entry = tx_service.trans.entry(tx.tx_id);
        let vacant_entry = match trans_entry {
            Entry::Occupied(_) => return Err(AccountServiceError::TransactionDuplicate),
            Entry::Vacant(entry) => entry,
        };

        account.deposit(amount)?;

        // only valid transactions are stored
        vacant_entry.insert(TransactionWithState {
            tx,
            state: TransactionState::Valid,
        });
        Ok(())
    }

    fn withdrawal(
        account: &mut Account,
        tx_service: &mut TransactionService,
        tx: Transaction,
    ) -> Result<(), AccountServiceError> {
        let amount = match tx.amount {
            Some(v) => v,
            None => return Err(AccountServiceError::EmptyTransactionAmount),
        };

        let trans_entry = tx_service.trans.entry(tx.tx_id);
        let _vacant_entry = match trans_entry {
            Entry::Occupied(_) => return Err(AccountServiceError::TransactionDuplicate),
            Entry::Vacant(entry) => entry,
        };

        if account.available < amount {
            return Err(AccountServiceError::InsufficientBalance);
        }
        account.available -= amount;

        // skip storing withdrawal as they are not disputable in this implementation
        // vacant_entry.insert(TransactionWithState{ tx, state: TransactionState::Valid });
        Ok(())
    }

    fn dispute(
        account: &mut Account,
        tx_service: &mut TransactionService,
        tx: Transaction,
    ) -> Result<(), AccountServiceError> {
        if tx.amount.is_some() {
            return Err(AccountServiceError::TransactionAmountShouldBeEmpty);
        };

        let prev_tx_state = tx_service.get_mut(tx.tx_id)?;
        let prev_tx = &prev_tx_state.tx;

        check_client(prev_tx, &tx)?;
        match prev_tx_state.state {
            TransactionState::Disputed => return Ok(()), // skip already disputed (duplicated transaction?)
            TransactionState::Refunded => Err(AccountServiceError::AlreadyRefunded),
            TransactionState::Valid => Ok(()),
        }?;

        if prev_tx.tx_type != TransactionType::Deposit {
            return Err(AccountServiceError::DisputeWrongTransactionType(
                prev_tx.tx_type,
            ));
        }

        let amount = match prev_tx.amount {
            Some(v) => v,
            None => return Err(AccountServiceError::EmptyTransactionAmount),
        };

        account.held(amount)?;
        prev_tx_state.state = TransactionState::Disputed;

        Ok(())
    }

    fn resolve(
        account: &mut Account,
        tx_service: &mut TransactionService,
        tx: Transaction,
    ) -> Result<(), AccountServiceError> {
        if tx.amount.is_some() {
            return Err(AccountServiceError::TransactionAmountShouldBeEmpty);
        };

        let prev_tx_state = tx_service.get_mut(tx.tx_id)?;
        let prev_tx = &prev_tx_state.tx;

        check_client(prev_tx, &tx)?;
        // skip not disputed or already solved dispute
        if prev_tx_state.state != TransactionState::Disputed {
            return Ok(());
        }

        let amount = match prev_tx.amount {
            Some(v) => v,
            None => return Err(AccountServiceError::EmptyTransactionAmount),
        };

        account.resolve(amount)?;
        // can be disputed again
        prev_tx_state.state = TransactionState::Valid;

        Ok(())
    }

    fn chargeback(
        account: &mut Account,
        tx_service: &mut TransactionService,
        tx: Transaction,
    ) -> Result<(), AccountServiceError> {
        if tx.amount.is_some() {
            return Err(AccountServiceError::TransactionAmountShouldBeEmpty);
        };

        let prev_tx_state = tx_service.get_mut(tx.tx_id)?;
        let prev_tx = &prev_tx_state.tx;

        check_client(prev_tx, &tx)?;
        // skip not disputed or already solved dispute
        if prev_tx_state.state != TransactionState::Disputed {
            return Ok(());
        }

        let amount = match prev_tx.amount {
            Some(v) => v,
            None => return Err(AccountServiceError::EmptyTransactionAmount),
        };

        if account.held < amount {
            return Err(AccountServiceError::InsufficientHeldBalance);
        }

        account.held -= amount;
        account.locked = true;
        prev_tx_state.state = TransactionState::Refunded;
        Ok(())
    }
}

fn check_client(prev_tx: &Transaction, tx: &Transaction) -> Result<(), AccountServiceError> {
    if prev_tx.client_id != prev_tx.client_id {
        return Err(AccountServiceError::MismatchedClient(
            tx.client_id,
            prev_tx.client_id,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::tx::TransactionType;

    #[test]
    fn resolve_dispute_and_open_dispute_again() {
        let mut accounts = AccountService::new();
        let mut tx_service = TransactionService::new();

        let deposit_trans = Transaction {
            tx_id: 13,
            tx_type: TransactionType::Deposit,
            client_id: 7,
            amount: Some(1000),
        };
        TransactionProcessor::process(&mut accounts, &mut tx_service, deposit_trans).unwrap();

        let dispute_trans = Transaction {
            tx_id: deposit_trans.tx_id,
            tx_type: TransactionType::Dispute,
            client_id: 7,
            amount: None,
        };
        TransactionProcessor::process(&mut accounts, &mut tx_service, dispute_trans).unwrap();
        let account = accounts.ensure_account(7);
        assert_eq!(0, account.available);
        assert_eq!(1000, account.held);

        let resolve_trans = Transaction {
            tx_id: deposit_trans.tx_id,
            tx_type: TransactionType::Resolve,
            client_id: 7,
            amount: None,
        };
        TransactionProcessor::process(&mut accounts, &mut tx_service, resolve_trans).unwrap();
        let account = accounts.ensure_account(7);
        assert_eq!(1000, account.available);
        assert_eq!(0, account.held);

        // dispute again
        TransactionProcessor::process(&mut accounts, &mut tx_service, dispute_trans).unwrap();
        TransactionProcessor::process(&mut accounts, &mut tx_service, dispute_trans).unwrap();

        let account = accounts.ensure_account(7);
        assert_eq!(0, account.available);
        assert_eq!(1000, account.held);

        let refound_trans = Transaction {
            tx_id: deposit_trans.tx_id,
            tx_type: TransactionType::Chargeback,
            client_id: 7,
            amount: None,
        };

        TransactionProcessor::process(&mut accounts, &mut tx_service, refound_trans).unwrap();

        let result = TransactionProcessor::process(&mut accounts, &mut tx_service, dispute_trans);
        let expected = Err(AccountServiceError::AccountLocked);
        assert_eq!(expected, result);

        let account = accounts.ensure_account(7);
        assert_eq!(0, account.available);
        assert_eq!(0, account.held);
    }

    #[test]
    fn tx_not_found_error() {
        let mut accounts = AccountService::new();
        let mut tx_service = TransactionService::new();

        let refound_trans = Transaction {
            tx_id: 13,
            tx_type: TransactionType::Chargeback,
            client_id: 7,
            amount: None,
        };

        let result = TransactionProcessor::process(&mut accounts, &mut tx_service, refound_trans);
        let expected = Err(AccountServiceError::TransactionNotFound);
        assert_eq!(expected, result);

        let account = accounts.ensure_account(7);
        assert_eq!(0, account.available);
        assert_eq!(0, account.held);

        let deposit_trans = Transaction {
            tx_id: 13,
            tx_type: TransactionType::Deposit,
            client_id: 7,
            amount: Some(1000),
        };
        TransactionProcessor::process(&mut accounts, &mut tx_service, deposit_trans).unwrap();

        let account = accounts.ensure_account(7);
        assert_eq!(1000, account.available);
        assert_eq!(0, account.held);

        // should be skipped
        TransactionProcessor::process(&mut accounts, &mut tx_service, refound_trans).unwrap();
        let account = accounts.ensure_account(7);
        assert_eq!(1000, account.available);
        assert_eq!(0, account.held);

        let dispute_trans = Transaction {
            tx_id: deposit_trans.tx_id,
            tx_type: TransactionType::Dispute,
            client_id: 7,
            amount: None,
        };
        TransactionProcessor::process(&mut accounts, &mut tx_service, dispute_trans).unwrap();

        TransactionProcessor::process(&mut accounts, &mut tx_service, refound_trans).unwrap();
        let account = accounts.ensure_account(7);
        assert_eq!(0, account.available);
        assert_eq!(0, account.held);
        assert_eq!(true, account.locked);
    }
}
