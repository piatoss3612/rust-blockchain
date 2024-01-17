use anyhow::anyhow;
use bincode::{deserialize, serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::block::Block;
use crate::errors::Result;
use crate::transaction::{TXOutputs, Transaction};

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
        let hash = match db.get("LAST")? {
            Some(h) => h.to_vec(),
            None => Vec::new(),
        };

        let lasthash = if hash.is_empty() {
            String::new()
        } else {
            String::from_utf8(hash)?
        };

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
    pub fn verify_transaction(&self, tx: &Transaction) -> Result<bool> {
        // coinbase transactions are always valid
        if tx.is_coinbase() {
            return Ok(true);
        }

        // get previous transactions referenced in the transaction (inputs)
        let prev_txs = self.get_prev_txs(tx)?;

        // verify the transaction
        tx.verify(prev_txs)
    }

    // Create a new Blockchain with a genesis block
    // address: the address to send the genesis block reward to
    pub fn create_blockchain(address: String) -> Result<Self> {
        // check if the blockchain already exists
        if Path::new("data/blocks").is_dir() {
            return Err(anyhow!("Blockchain already exists"));
        }

        // open the database
        let db = sled::open("data/blocks")?;

        // create a coinbase transaction
        let cbtx = Transaction::new_coinbase(address, String::from(GENESIS_COINBASE_DATA))?;

        // create a genesis block
        let genesis: Block = Block::new_genesis_block(cbtx);

        // insert the genesis block into the database
        db.insert(genesis.get_hash(), serialize(&genesis)?)
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

    // Mine a new block with the provided transactions
    // transactions: the transactions to include in the block
    pub fn mine_block(&mut self, transactions: Vec<Transaction>) -> Result<Block> {
        // verify the transactions before mining
        for tx in &transactions {
            if !self.verify_transaction(tx)? {
                return Err(anyhow!("Invalid transaction"));
            }
        }

        // get the hash of the last block
        let lasthash = match self.db.get("LAST")? {
            Some(h) => h.to_vec(),
            None => Err(anyhow!("Last hash not found"))?,
        };

        // create a new block with the transactions, the hash of the last block and the best block height
        let new_block = Block::new_block(
            transactions,
            String::from_utf8(lasthash)?,
            self.get_best_height()?,
        )?;

        // insert the new block into the database
        self.db
            .insert(new_block.get_hash(), serialize(&new_block)?)?;
        self.db.insert("LAST", new_block.get_hash().as_bytes())?;
        self.db.flush()?;

        self.current_hash = new_block.get_hash();

        // return the new block
        Ok(new_block)
    }

    // Add a block to the blockchain
    // block: the block to add
    pub fn add_block(&mut self, block: Block) -> Result<()> {
        // Serialize the block
        let data = serialize(&block)?;

        // Check if the block already exists
        if let Some(_) = self.db.get(block.get_hash())? {
            return Ok(());
        }

        // Insert the block into the database
        self.db.insert(block.get_hash(), data)?;

        let height = self.get_best_height()?;

        if block.get_height() > height {
            self.db.insert("LAST", block.get_hash().as_bytes())?;
            self.current_hash = block.get_hash();
            self.db.flush()?;
        }

        // Return Ok
        Ok(())
    }

    // Get a block by its hash
    pub fn get_block(&self, hash: &str) -> Result<Block> {
        // Get the block from the database
        let data = match self.db.get(hash)? {
            Some(d) => d,
            None => Err(anyhow!("Block not found"))?,
        }
        .to_vec();

        // Deserialize the block
        let block = deserialize::<Block>(&data)?;

        // Return the block
        Ok(block)
    }

    // Get the best block height
    pub fn get_best_height(&self) -> Result<u32> {
        // Get the hash of the last block
        let lasthash = match self.db.get("LAST")? {
            Some(h) => h.to_vec(),
            None => Err(anyhow!("Last hash not found"))?,
        };

        // Get the last block from the database
        let data = match self.db.get(String::from_utf8(lasthash)?)? {
            Some(d) => d,
            None => Err(anyhow!("Block not found"))?,
        };

        // Deserialize the block
        let block = deserialize::<Block>(&data.to_vec())?;

        // Return the height of the block
        Ok(block.get_height())
    }

    // Get the hash of all blocks from the last to the first
    pub fn get_block_hashs(&self) -> Vec<String> {
        // Create a vector to store the hashs
        let mut hashs = Vec::new();

        // Iterate over all blocks in the blockchain
        for block in self.iter() {
            hashs.push(block.get_hash());
        }

        // Return the hashs
        hashs
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

    // Create a new BlockchainIteratorator
    pub fn iter(&self) -> BlockchainIterator {
        BlockchainIterator {
            current_hash: self.current_hash.clone(),
            bc: &self,
        }
    }
}

// BlockchainIterator struct contains a current hash and a reference to a Blockchain
// It implements Iterator trait and has lifetime 'a (which means it can't outlive the Blockchain it refers to)
pub struct BlockchainIterator<'a> {
    current_hash: String,
    bc: &'a Blockchain,
}

impl<'a> Iterator for BlockchainIterator<'a> {
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
