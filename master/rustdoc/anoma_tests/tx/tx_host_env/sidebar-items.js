initSidebarItems({"attr":[["transaction","Generate WASM binding for a transaction main entrypoint function."]],"constant":[["ENV","A [`TestTxEnv`] that can be used for tx host env functions calls that implements the WASM host environment in native environment."]],"derive":[["BorshDeserialize",""],["BorshSerialize",""]],"enum":[["Address","An account’s address"]],"fn":[["commit_tx_and_block",""],["delete","Delete a value at the given key from storage."],["emit_ibc_event","Emit an IBC event. There can be only one event per transaction. On multiple calls, only the last emitted event will be used."],["get_block_epoch","Get epoch of the current block"],["get_block_hash","Get hash of the current block"],["get_block_height","Get height of the current block"],["get_block_time","Get time of the current block header as rfc 3339 string"],["get_chain_id","Get the chain ID"],["has_key","Check if the given key is present in storage."],["init","Initialize the tx host environment in [`ENV`]. This will be used in the host env function calls via macro `native_host_fn!`."],["init_account",""],["insert_verifier","Insert a verifier address. This address must exist on chain, otherwise the transaction will be rejected."],["iter_prefix","Get an iterator with the given prefix."],["log_string","Log a string. The message will be printed at the `tracing::Level::Info`."],["read","Try to read a Borsh encoded variable-length value at the given key from storage."],["read_bytes","Try to read a variable-length value as bytes at the given key from storage."],["set","Set the tx host environment in [`ENV`] from the given [`TestTxEnv`]. This will be used in the host env function calls via macro `native_host_fn!`."],["take","Take the [`TestTxEnv`] out of [`ENV`]. The [`ENV`] must be initialized."],["update_validity_predicate","Update a validity predicate"],["with","Mutably borrow the [`TestTxEnv`] from [`ENV`]. The [`ENV`] must be initialized."],["write","Write a value to be encoded with Borsh at the given key to storage."],["write_bytes","Write a value as bytes at the given key to storage."],["write_bytes_temp","Write a temporary value as bytes at the given key to storage."],["write_temp","Write a temporary value to be encoded with Borsh at the given key to storage."]],"mod":[["address","Implements transparent addresses as described in Accounts Addresses."],["chain","Chain related data types"],["dylib","Dynamic library helpers"],["governance","Tx imports and functions."],["hash","Types for working with 32 bytes hashes."],["ibc","Types that are used in IBC."],["intent","Tx imports and functions."],["internal","Shared internal types between the host env and guest (wasm)."],["key","Cryptographic keys"],["matchmaker","Matchmaker types"],["nft","Tx imports and functions."],["proof_of_stake","Proof of Stake system integration with functions for transactions"],["storage","Storage types"],["time","Types for dealing with time and durations."],["token","Tx imports and functions."],["transaction","Types that are used in transactions."],["validity_predicate","Types that are used in validity predicates."]],"struct":[["Ibc","This struct integrates and gives access to lower-level IBC functions."],["KeyValIterator",""],["PoS","Proof of Stake system. This struct integrates and gives access to lower-level PoS functions."],["Signed","A generic signed data wrapper for Borsh encode-able data."],["SignedTxData","This can be used to sign an arbitrary tx. The signature is produced and verified on the tx data concatenated with the tx code, however the tx code itself is not part of this structure."]],"trait":[["BorshDeserialize","A data-structure that can be de-serialized from binary format by NBOR."],["BorshSerialize","A data-structure that can be serialized into binary format by NBOR."],["IbcActions","IBC trait to be implemented in integration that can read and write"],["PosRead","Read-only part of the PoS system"],["PosWrite","PoS system trait to be implemented in integration that can read and write PoS data."]]});