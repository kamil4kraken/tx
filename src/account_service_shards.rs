use crate::account_service::AccountService;
use crate::tx::Transaction;
use crate::tx_processor::TransactionProcessor;
use crate::tx_service::TransactionService;

use async_channel;
use futures_lite::future;
use std::sync::{Arc, Mutex};
use std::thread;

// single channel capacity
const CHANNEL_CAP: usize = 256;

pub struct AccountShards {
    shards: usize,
    // TODO: implement Iterator adaptor and remove 'pub'
    pub account_services: Vec<Arc<Mutex<AccountService>>>,
    tx_services: Vec<Arc<Mutex<TransactionService>>>,

    // channels to pass transactions to threads/shards
    channels: Vec<(
        async_channel::Sender<Transaction>,
        async_channel::Receiver<Transaction>,
    )>,
    handles: Vec<thread::JoinHandle<()>>,
}

impl AccountShards {
    pub fn new(shards: usize) -> Self {
        let mut new_shards = Self {
            shards,
            account_services: Vec::with_capacity(shards),
            tx_services: Vec::with_capacity(shards),
            channels: Vec::with_capacity(shards),
            handles: Vec::with_capacity(shards),
        };
        for _i in 0..shards {
            new_shards
                .account_services
                .push(Arc::new(Mutex::new(AccountService::new())));
            new_shards
                .tx_services
                .push(Arc::new(Mutex::new(TransactionService::new())));
            new_shards
                .channels
                .push(async_channel::bounded(CHANNEL_CAP));
        }
        new_shards
    }

    pub fn run(&mut self) {
        for i in 0..self.shards {
            let a_service = Arc::clone(&self.account_services[i]);
            let t_service = Arc::clone(&self.tx_services[i]);
            let receiver = self.channels[i].1.clone();

            self.handles.push(thread::spawn(move || {
                let mut a_service = a_service.lock().unwrap();
                let mut t_service = t_service.lock().unwrap();

                while let Ok(tx) = future::block_on(receiver.recv()) {
                    if let Err(err) =
                        TransactionProcessor::process(&mut a_service, &mut t_service, tx)
                    {
                        eprintln!("Transaction {} failed: {}", tx.tx_id, err);
                    }
                }
            }));
        }
    }

    pub fn join(&mut self) {
        // close channels, the remaining messages can still be received
        for q in self.channels.iter() {
            q.0.close();
        }

        // wait for all threads to finish
        while let Some(handle) = self.handles.pop() {
            handle.join().unwrap();
        }
    }

    pub fn process(&mut self, tx: Transaction) {
        // because number of workers can change in the future would be better to use consistent hashing
        let hash = (tx.client_id as usize) % self.shards;
        future::block_on(self.channels[hash].0.send(tx)).unwrap();
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::tx::*;
    use rand::Rng;

    #[test]
    fn deposit_open_dispute_and_than_resolve() {
        let mut shards = AccountShards::new(16);
        assert_eq!(shards.shards, 16);
        assert_eq!(shards.account_services.len(), 16);
        shards.run();

        let mut rng = rand::thread_rng();

        for i in 0..10_000 {
            let tx = Transaction {
                tx_id: i,
                tx_type: TransactionType::Deposit,
                client_id: i as u16,
                amount: Some(1000 * rng.gen::<u32>() as AmountDecimal),
            };
            shards.process(tx);
        }
        
        for i in 10_000..20_000 {
            let tx = Transaction {
                tx_id: i,
                tx_type: TransactionType::Withdrawal,
                client_id: (i - 10_000) as u16,
                amount: Some((rng.gen::<u16>() % 1000) as AmountDecimal),
            };
            shards.process(tx);
        }

        for i in 20_000..30_000 {
            let tx = Transaction {
                tx_id: i,
                tx_type: TransactionType::Deposit,
                client_id: i as u16 - 20_000,
                amount: Some(100 * rng.gen::<u32>() as AmountDecimal),
            };
            shards.process(tx);
        }

        for i in 0..10_000 {
            let tx = Transaction {
                tx_id: i,
                tx_type: TransactionType::Dispute,
                client_id: i as u16,
                amount: None,
            };
            shards.process(tx);
        }

        for i in 0..5_000 {
            let tx = Transaction {
                tx_id: i,
                tx_type: TransactionType::Chargeback,
                client_id: i as u16,
                amount: None,
            };
            shards.process(tx);
        }

        for i in 5_000..10_000 {
            let tx = Transaction {
                tx_id: i,
                tx_type: TransactionType::Resolve,
                client_id: i as u16,
                amount: None,
            };
            shards.process(tx);
        }

        shards.join();
    }
}
