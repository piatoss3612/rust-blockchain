use std::collections::HashMap;

use anyhow::anyhow;

use crate::block::Block;
use crate::errors::Result;
use crate::transaction::{TXOutputs, Transaction};

const TARGET_HEXT: usize = 4; // 4 leading zeros in hex (difficulty)
const GENESIS_COINBASE_DATA: &str =
    "The Times 03/Jan/2009 Chancellor on brink of second bailout for banks"; // genesis block data

// Blockchain struct contains a current hash and a database
#[derive(Debug, Clone)]
pub struct Blockchain {
    current_hash: String, // hash of the last block
    db: sled::Db,         // database
}

impl Blockchain {
    // Create a blockchain instance
    pub fn new() -> Result<Self> {
        // open the database
        let db = sled::open("data/blocks")?;

        // get the hash of the last block
        let hash = db.get("LAST")?.expect("No existing blockchain found");
        let lasthash = String::from_utf8(hash.to_vec())?;

        // return the Blockchain
        Ok(Self {
            current_hash: lasthash,
            db,
        })
    }

    // Sign a transaction with a private key
    // tx: the transaction to sign
    // priate_key: the private key to sign the transaction with
    pub fn sign_transaction(&self, tx: &mut Transaction, priate_key: &[u8]) -> Result<()> {
        // get previous transactions referenced in the transaction (inputs)
        let prev_txs = self.get_prev_txs(tx)?;

        // sign the transaction with the private key and previous transactions
        tx.sign(priate_key, prev_txs)?;

        // return Ok
        Ok(())
    }

    // Find previous transactions referenced in the transaction
    // tx: the transaction to verify
    pub fn get_prev_txs(&self, tx: &Transaction) -> Result<HashMap<String, Transaction>> {
        let mut prev_txs = HashMap::new();

        // Find previous transactions referenced in the transaction (inputs)
        for vin in &tx.vin {
            let prev_tx = self.find_transaction(&vin.txid)?;
            prev_txs.insert(prev_tx.id.clone(), prev_tx);
        }

        // return previous transactions
        Ok(prev_txs)
    }

    // Find a transaction by its ID
    // id: the ID of the transaction to find
    pub fn find_transaction(&self, id: &str) -> Result<Transaction> {
        // iterate over the blockchain
        for block in self.iter() {
            for tx in block.get_transactions() {
                if tx.id == id {
                    return Ok(tx.clone());
                }
            }
        }

        // return an error if the transaction is not found
        Err(anyhow!("Transaction is not found"))
    }

    // Verify a transaction
    pub fn verify_transaction(&self, tx: &mut Transaction) -> Result<bool> {
        // get previous transactions referenced in the transaction (inputs)
        let prev_txs = self.get_prev_txs(tx)?;

        // verify the transaction
        tx.verify(prev_txs)
    }

    // Create a new Blockchain with a genesis block
    // address: the address to send the genesis block reward to
    pub fn create_blockchain(address: String) -> Result<Self> {
        // open the database
        let db = sled::open("data/blocks")?;

        // create a coinbase transaction
        let cbtx = Transaction::new_coinbase(address, String::from(GENESIS_COINBASE_DATA))?;

        // create a genesis block
        let genesis: Block = Block::new_genesis_block(cbtx);

        // insert the genesis block into the database
        db.insert(genesis.get_hash(), bincode::serialize(&genesis)?)
            .expect("Failed to insert");
        db.insert("LAST", genesis.get_hash().as_bytes())?;

        // flush the database
        db.flush()?;

        // return the Blockchain
        Ok(Self {
            current_hash: genesis.get_hash(),
            db,
        })
    }

    // Add a block to the blockchain
    // data: the data to add to the block
    pub fn add_block(&mut self, data: Vec<Transaction>) -> Result<Block> {
        // get the hash of the last block
        let lasthash = self.db.get("LAST")?.unwrap();

        // create a new block with the data, the hash of the last block and the target
        let new_block = Block::new_block(data, String::from_utf8(lasthash.to_vec())?, TARGET_HEXT)?;

        // insert the new block into the database
        self.db
            .insert(new_block.get_hash(), bincode::serialize(&new_block)?)?;
        self.db.insert("LAST", new_block.get_hash().as_bytes())?;

        // update the current hash of the blockchain
        self.current_hash = new_block.get_hash();

        // return the new block
        Ok(new_block)
    }

    // Find all unspent transaction outputs and return transactions with spent outputs removed
    pub fn find_utxo(&self) -> HashMap<String, TXOutputs> {
        let mut utxos: HashMap<String, TXOutputs> = HashMap::new();
        let mut spent_txos: HashMap<String, Vec<i32>> = HashMap::new();

        // Iterate over all blocks in the blockchain
        for block in self.iter() {
            // Iterate over all transactions in the block
            for tx in block.get_transactions() {
                // Iterate over all outputs in the transaction
                for idx in 0..tx.vout.len() {
                    // Check if output is already in unspent outputs and if so, skip it
                    if let Some(ids) = spent_txos.get(&tx.id) {
                        if ids.contains(&(idx as i32)) {
                            continue;
                        }
                    }

                    // Update utxos
                    match utxos.get_mut(&tx.id) {
                        Some(v) => {
                            v.outputs.push(tx.vout[idx].clone());
                        }
                        None => {
                            utxos.insert(
                                tx.id.clone(),
                                TXOutputs {
                                    outputs: vec![tx.vout[idx].clone()],
                                },
                            );
                        }
                    }
                }

                // If the transaction is not a coinbase transaction, add its inputs to spent_txos
                // because they are now spent and can't be spent again
                if !tx.is_coinbase() {
                    for vin in &tx.vin {
                        match spent_txos.get_mut(&vin.txid) {
                            Some(v) => {
                                v.push(vin.vout);
                            }
                            None => {
                                spent_txos.insert(vin.txid.clone(), vec![vin.vout]);
                            }
                        }
                    }
                }
            }

            /*
               1. Iterate over all transactions in the block from the last to the first
               2. Iterate over all outputs in the transaction and filter out spent outputs
               3. If the transaction is not a coinbase transaction, add its inputs to spent_txos because they are now spent and can't be spent again
               4. Repeat 1-3 for all blocks in the blockchain
            */
        }

        // Return unspent transaction outputs
        utxos
    }

    // Create a new BlockchainIterator
    pub fn iter(&self) -> BlockchainIter {
        BlockchainIter {
            current_hash: self.current_hash.clone(),
            bc: &self,
        }
    }
}

// BlockchainIter struct contains a current hash and a reference to a Blockchain
// It implements Iterator trait and has lifetime 'a (which means it can't outlive the Blockchain it refers to)
pub struct BlockchainIter<'a> {
    current_hash: String,
    bc: &'a Blockchain,
}

impl<'a> Iterator for BlockchainIter<'a> {
    type Item = Block; // The type of the data that iterates over

    // Get the next item in the iterator
    fn next(&mut self) -> Option<Self::Item> {
        if let Ok(encode_block) = self.bc.db.get(&self.current_hash) {
            return match encode_block {
                Some(b) => {
                    // Deserialize the block and set the current hash to the previous hash
                    if let Ok(block) = bincode::deserialize::<Block>(&b) {
                        self.current_hash = block.get_prev_hash();

                        // Return the block
                        Some(block)
                    } else {
                        None
                    }
                }
                None => None,
            };
        }
        None
    }
}
