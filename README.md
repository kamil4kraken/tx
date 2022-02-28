first code attempt in rust, coming from c++/go

Assumptions:
- withdraws are not disputable
- dispute / resolve / chargeback can be processed in full amount or rejected
- transaction duplicates (same tx) with matching client are ignored
- skip errors like dispute after dispute or chargeback not disputed transaction
- print error on stderr when trying to chargeback resolved or already refunded transaction
- amount must be empty for dispute/resolve/chargeback
- client has to match ie. for deposit and dispute
- transactions and accounts can fit into RAM memory

Transactions are read from csv file using iterator by main thread and processed in shards (size = #cpu) using client_id as shard key.
Worker threads receive transactions from channels and store account and transaction history/state in-memory.
Transactions and accounts in this implementation are stored in simple hashmap (without persistence and without write-ahead logging #TODO).
In-memory storage instance is per shard/thread (no locks are needed during transaction processing).
