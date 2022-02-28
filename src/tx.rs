use serde::Deserialize;
use strum_macros::EnumString;

pub type ClientId = u16;
pub type TransactionId = u32;

// store coins as value * base
pub type AmountDecimal = u64;
pub const AMOUNT_BASE: u16 = 1000;

#[derive(EnumString, Debug, Copy, Clone, PartialEq, Deserialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, Deserialize, Copy, Clone)]
pub struct Transaction {
    #[serde(rename = "type")]
    pub tx_type: TransactionType,

    #[serde(rename = "client")]
    pub client_id: ClientId,

    #[serde(rename = "tx")]
    pub tx_id: TransactionId,

    #[serde(with = "amount_decimal")]
    pub amount: Option<AmountDecimal>,
}

mod amount_decimal {
    use super::*;
    use serde::{self, Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<AmountDecimal>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s.is_empty() {
            return Ok(None);
        }

        let f: f64 = s.parse().map_err(serde::de::Error::custom)?;
        if f < 0.0 {
            return Err(serde::de::Error::custom(&format!(
                "Unexpected amount value: {}",
                f
            )));
        }
        Ok(Some((f * (AMOUNT_BASE as f64)) as AmountDecimal))
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::str::FromStr;

    #[test]
    fn type_is_comparable() {
        assert_eq!(
            TransactionType::from_str("resolve").unwrap(),
            TransactionType::Resolve
        );
        assert_ne!(
            TransactionType::from_str("dispute").unwrap(),
            TransactionType::Resolve
        );
    }
}
