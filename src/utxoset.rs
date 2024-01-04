use crate::block::Block;
use crate::blockchain::Blockchain;
use crate::errors::Result;
use crate::transaction::TXOutputs;
use log::info;
use std::collections::HashMap;

/// UTXOSet struct contains a Blockchain
pub struct UTXOSet {
    pub blockchain: Blockchain,
}

impl UTXOSet {
    // Rebuild the UTXO set from blockchain
    pub fn reindex(&self) -> Result<()> {
        // Remove old UTXO set if it exists
        if let Err(e) = std::fs::remove_dir_all("data/utxos") {
            info!("remove_dir_all error: {}", e);
        }

        // Create a new UTXO set
        let db = sled::open("data/utxos")?;

        // Find all unspent transaction outputs and add them to UTXO set
        let utxos = self.blockchain.find_utxo();

        for (txid, outs) in utxos {
            db.insert(txid.as_bytes(), bincode::serialize(&outs)?)?;
        }

        Ok(())
    }
    // Find all unspent transaction outputs and return transactions with spent outputs removed
    // address: the address to find unspent transaction outputs for
    // amount: the amount needed
    pub fn find_spendable_outputs(
        &self,
        address: &[u8],
        amount: i32,
    ) -> Result<(i32, HashMap<String, Vec<i32>>)> {
        // Declare a HashMap to store unspent outputs
        let mut unspent_outputs: HashMap<String, Vec<i32>> = HashMap::new();

        // Declare a variable to store accumulated amount of unspent outputs
        let mut accumulated = 0;

        // Open the UTXO set database
        let db = sled::open("data/utxos")?;

        // Iterate over all unspent transaction outputs
        'out: for kv in db.iter() {
            let (k, v) = kv?;

            // Parse transaction ID and its outputs
            let txid = String::from_utf8(k.to_vec())?;
            let outs: TXOutputs = bincode::deserialize(&v.to_vec())?;

            for idx in 0..outs.outputs.len() {
                // Check if output is locked with given address and if so, add it to unspent outputs
                if outs.outputs[idx].is_locked_with_key(address) && accumulated < amount {
                    accumulated += outs.outputs[idx].value;

                    // Add transaction ID and output index to unspent outputs HashMap
                    // If transaction ID already exists in HashMap, push output index to its vector
                    match unspent_outputs.get_mut(&txid) {
                        Some(v) => v.push(idx as i32),
                        None => {
                            unspent_outputs.insert(txid.clone(), vec![idx as i32]);
                        }
                    }
                }

                // Check if accumulated amount is enough and if so, break out of loop
                if accumulated >= amount {
                    break 'out;
                }
            }
        }

        // Return accumulated amount and unspent outputs
        Ok((accumulated, unspent_outputs))
    }

    // Find all unspent transaction outputs and return transactions with spent outputs removed
    // pub_key_hash: the public key hash to find unspent transaction outputs for
    pub fn find_utxo(&self, pub_key_hash: &[u8]) -> Result<TXOutputs> {
        // Declare a TXOutputs struct to store unspent outputs
        let mut utxos = TXOutputs {
            outputs: Vec::new(),
        };

        // Open the UTXO set database
        let db = sled::open("data/utxos")?;

        for kv in db.iter() {
            let (_, v) = kv?;

            // Parse transaction outputs
            let outs: TXOutputs = bincode::deserialize(&v.to_vec())?;

            // Iterate over transaction outputs and check if they are locked with given public key hash
            for out in outs.outputs {
                if out.is_locked_with_key(pub_key_hash) {
                    utxos.outputs.push(out.clone())
                }
            }
        }

        // Return unspent outputs
        Ok(utxos)
    }

    // Update the UTXO set with transactions from the Block
    // block: the Block to update the UTXO set with
    // TODO - improve this function
    pub fn update(&self, block: &Block) -> Result<()> {
        // Open the UTXO set database
        let db = sled::open("data/utxos")?;

        for tx in block.get_transactions() {
            // If transaction is not a coinbase transaction, iterate over its inputs and remove them from UTXO set
            if !tx.is_coinbase() {
                // Iterate over transaction inputs
                for vin in &tx.vin {
                    let mut update_outputs = TXOutputs {
                        outputs: Vec::new(),
                    };

                    // Get transaction outputs for transaction ID
                    let outs: TXOutputs =
                        bincode::deserialize(&db.get(&vin.txid)?.unwrap().to_vec())?;

                    // Iterate over transaction outputs and add them to update_outputs except for the one that is being spent
                    for out_idx in 0..outs.outputs.len() {
                        if out_idx != vin.vout as usize {
                            update_outputs.outputs.push(outs.outputs[out_idx].clone());
                        }
                    }

                    // If there are no more outputs for the transaction ID, remove it from UTXO set
                    // Otherwise, update it with the new outputs
                    if update_outputs.outputs.is_empty() {
                        db.remove(&vin.txid)?;
                    } else {
                        db.insert(vin.txid.as_bytes(), bincode::serialize(&update_outputs)?)?;
                    }
                }
            }

            // Declare a new TXOutputs struct to store transaction outputs
            let mut new_outputs = TXOutputs {
                outputs: Vec::new(),
            };

            // Iterate over transaction outputs and add them to new_outputs
            for out in &tx.vout {
                new_outputs.outputs.push(out.clone());
            }

            // Add transaction ID and new_outputs to UTXO set
            db.insert(tx.id.as_bytes(), bincode::serialize(&new_outputs)?)?;
        }

        // Return Ok
        Ok(())
    }

    // Count the number of transactions in the UTXO set
    pub fn count_transactions(&self) -> Result<i32> {
        let mut counter = 0;

        // Open the UTXO set database
        let db = sled::open("data/utxos")?;

        // Iterate over all transactions in UTXO set
        for kv in db.iter() {
            kv?;
            counter += 1;
        }

        // Return counter
        Ok(counter)
    }
}
