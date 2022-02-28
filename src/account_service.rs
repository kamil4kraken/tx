use crate::tx::*;

use serde::Serialize;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
pub struct Account {
    pub client_id: ClientId,
    pub available: AmountDecimal,
    pub held: AmountDecimal,
    pub locked: bool,
}

type AccountStorage = HashMap<ClientId, Account>;

pub struct AccountService {
    accounts: AccountStorage,
}

// in-memory account storage
impl AccountService {
    pub fn new() -> Self {
        Self {
            accounts: AccountStorage::default(),
        }
    }

    pub fn ensure_account(&mut self, client_id: ClientId) -> &mut Account {
        let account = self
            .accounts
            .entry(client_id)
            .or_insert_with(|| Account::new(client_id, 0));
        account
    }

    pub fn iter(&self) -> AccountIter {
        AccountIter {
            inner: self.accounts.values(),
        }
    }
}

// should provide 'atomic' operations on account balance
impl Account {
    pub fn new(client_id: ClientId, available: AmountDecimal) -> Self {
        Self {
            client_id,
            available,
            held: 0,
            locked: false,
        }
    }

    pub fn deposit(&mut self, amount: AmountDecimal) -> Result<(), AccountServiceError> {
        checked_add(checked_add(self.available, self.held)?, amount)?;
        self.available += amount;
        Ok(())
    }

    pub fn held(&mut self, amount: AmountDecimal) -> Result<(), AccountServiceError> {
        if self.available < amount {
            // TODO max possible amount should be held ?? (this would complicate resolve/chargeback)
            return Err(AccountServiceError::InsufficientBalance);
        }
        self.held = checked_add(self.held, amount)?;
        self.available -= amount;
        Ok(())
    }

    pub fn resolve(&mut self, amount: AmountDecimal) -> Result<(), AccountServiceError> {
        if self.held < amount {
            return Err(AccountServiceError::InsufficientHeldBalance);
        }

        self.available = checked_add(self.available, amount)?;
        self.held -= amount;
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub enum AccountServiceError {
    BalanceOverflow,
    AccountLocked,
    TransactionNotFound,
    TransactionDuplicate,
    InsufficientBalance,
    AlreadyRefunded,
    DisputeWrongTransactionType(TransactionType),
    InsufficientHeldBalance,
    MismatchedClient(ClientId, ClientId),
    EmptyTransactionAmount,
    TransactionAmountShouldBeEmpty,
}

impl std::error::Error for AccountServiceError {}

impl fmt::Display for AccountServiceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO:
        // match self {
        //     AccountServiceError::TransactionNotFound =>
        //     ...
        //}
        write!(f, "AccountServiceError: {:?}", self)
    }
}

#[derive(Debug, Serialize)]
pub struct AccountResult {
    client: ClientId,
    #[serde(with = "amount_decimal")]
    available: AmountDecimal,
    #[serde(with = "amount_decimal")]
    held: AmountDecimal,
    #[serde(with = "amount_decimal")]
    total: AmountDecimal,
    locked: bool,
}

mod amount_decimal {
    use super::*;
    use serde::{self, Serializer};

    pub fn serialize<S>(value: &AmountDecimal, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        let f: f64 = (*value as f64) / (AMOUNT_BASE as f64);
        serializer.serialize_f64(f)
    }
}

pub struct AccountIter<'a> {
    inner: std::collections::hash_map::Values<'a, ClientId, Account>,
}

impl<'a> Iterator for AccountIter<'a> {
    type Item = AccountResult;

    fn next(&mut self) -> Option<Self::Item> {
        let a = self.inner.next()?;
        Some(AccountResult {
            client: a.client_id,
            available: a.available,
            held: a.held,
            total: a.available + a.held, // checked_add ?
            locked: a.locked,
        })
    }
}

pub fn checked_add(
    balance: AmountDecimal,
    val: AmountDecimal,
) -> Result<AmountDecimal, AccountServiceError> {
    let res = balance.checked_add(val);
    match res {
        None => Err(AccountServiceError::BalanceOverflow),
        Some(amount) => Ok(amount),
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn create_account() {
        let account: Account = Account::new(1, 0);
        assert_eq!(account.client_id, 1);
        assert_eq!(account.available, 0);
        assert_eq!(account.held, 0);
        assert_eq!(account.locked, false);
    }

    #[test]
    fn account_balance_balance() {
        let mut account: Account = Account::new(1, 0);
        assert_eq!(account.available, 0);
        account.deposit(100).unwrap();
        assert_eq!(account.available, 100);
        assert_eq!(account.held, 0);

        account.held(50).unwrap();
        assert_eq!(account.available, 50);
        assert_eq!(account.held, 50);

        account.resolve(50).unwrap();
        assert_eq!(account.available, 100);
        assert_eq!(account.held, 0);
    }
}
