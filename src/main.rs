use tx::account_service_shards;
use tx::tx_csv_iter;
use std::io;
use std::path::PathBuf;
use structopt::StructOpt;

extern crate num_cpus;

// questions / todo:
// move to hold max available amount when available < disputed amount
// or add hold_target to be increased by dispute when available < disputed amount and decreased when new deposit arrive?
// store rejected transactions (better errors when dispute arrive)
// allow to dispute withdrawals?
// tests documenting expected behaviour in cases above

#[derive(Debug, StructOpt)]
struct Opt {
    /// Input file
    #[structopt(parse(from_os_str), help = "transactions.csv")]
    input: PathBuf,
}

fn main() {
    let opt = Opt::from_args();

    let mut shards = account_service_shards::AccountShards::new(num_cpus::get());
    shards.run();
    let iter = tx_csv_iter::TransIterator::new(&opt.input).expect("Cannot open input file");
    iter.for_each(|tx| shards.process(tx));
    shards.join();

    let mut writer = csv::Writer::from_writer(io::stdout());
    for account_service in shards.account_services {
        for account in account_service.lock().unwrap().iter() {
            writer.serialize(account).expect("Account serialize error");
        }
    }
    writer.flush().expect("Print output error");
}
