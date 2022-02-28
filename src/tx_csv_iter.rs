use crate::tx::*;

use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

pub struct TransIterator {
    inner: csv::DeserializeRecordsIntoIter<BufReader<File>, Transaction>,
}

impl TransIterator {
    pub fn new(path: &PathBuf) -> Result<Self, Box<dyn Error>> {
        let f = File::open(path)?;
        let br = std::io::BufReader::new(f);
        Ok(TransIterator {
            inner: csv::ReaderBuilder::new()
                .trim(csv::Trim::All)
                .flexible(true)
                .from_reader(br)
                .into_deserialize(),
        })
    }
}

impl Iterator for TransIterator {
    type Item = Transaction;

    // inner iter, on error skip
    fn next(&mut self) -> Option<Transaction> {
        loop {
            match self.inner.next() {
                Some(v) => match v {
                    Ok(t) => return Some(t),
                    Err(e) => {
                        println!("Parse error {}", e);
                    } // on error skip
                },
                None => return None,
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::path::PathBuf;

    #[test]
    fn read_csv() {
        let path = PathBuf::from("./data/transactions.csv");
        let iter = TransIterator::new(&path).expect("Cannot open input file");
        let v: Vec<_> = iter.collect();
        assert_eq!(v.len(), 5);
    }

    #[test]
    fn read_csv_with_error() {
        let path = PathBuf::from("./data/transactions_wrong.csv");
        let iter = TransIterator::new(&path).expect("Cannot open input file");
        let v: Vec<_> = iter.collect();
        assert_eq!(v.len(), 5);
    }
}
