use std::collections::HashMap;

use anyhow::anyhow;

use crate::block::Block;
use crate::errors::Result;
use crate::transaction::{TXOutput, Transaction};

const TARGET_HEXT: usize = 4;
const GENESIS_COINBASE_DATA: &str =
    "The Times 03/Jan/2009 Chancellor on brink of second bailout for banks";

#[derive(Debug, Clone)]
pub struct Blockchain {
    current_hash: String,
    db: sled::Db,
}

pub struct BlockchainIter<'a> {
    current_hash: String,
    bc: &'a Blockchain,
}
impl Blockchain {
    pub fn new() -> Result<Blockchain> {
        let db = sled::open("data/blocks")?;
        let hash = db.get("LAST")?.expect("No existing blockchain found");
        let lasthash = String::from_utf8(hash.to_vec())?;
        Ok(Blockchain {
            current_hash: lasthash,
            db,
        })
    }

    pub fn sign_transaction(&self, tx: &mut Transaction, priate_key: &[u8]) -> Result<()> {
        let prev_txs = self.get_prev_txs(tx)?;
        tx.sign(priate_key, prev_txs)?;
        Ok(())
    }

    pub fn get_prev_txs(&self, tx: &Transaction) -> Result<HashMap<String, Transaction>> {
        let mut prev_txs = HashMap::new();
        for vin in &tx.vin {
            let prev_tx = self.find_transaction(&vin.txid)?;
            prev_txs.insert(prev_tx.id.clone(), prev_tx);
        }
        Ok(prev_txs)
    }

    pub fn find_transaction(&self, id: &str) -> Result<Transaction> {
        for block in self.iter() {
            for tx in block.get_transaction() {
                if tx.id == id {
                    return Ok(tx.clone());
                }
            }
        }
        Err(anyhow!("Transaction is not found"))
    }

    pub fn verify_transaction(&self, tx: &mut Transaction) -> Result<bool> {
        let prev_txs = self.get_prev_txs(tx)?;
        tx.verify(prev_txs)
    }

    pub fn create_blockchain(address: String) -> Result<Self> {
        let db = sled::open("data/blocks")?;
        let cbtx = Transaction::new_coinbase(address, String::from(GENESIS_COINBASE_DATA))?;
        let genesis: Block = Block::new_genesis_block(cbtx);
        db.insert(genesis.get_hash(), bincode::serialize(&genesis)?)?;
        db.insert("LAST", genesis.get_hash().as_bytes())?;
        let bc = Blockchain {
            current_hash: genesis.get_hash(),
            db,
        };
        bc.db.flush()?;
        Ok(bc)
    }

    pub fn add_block(&mut self, data: Vec<Transaction>) -> Result<()> {
        let lasthash = self.db.get("LAST")?.unwrap();

        let new_block = Block::new_block(data, String::from_utf8(lasthash.to_vec())?, TARGET_HEXT)?;
        self.db
            .insert(new_block.get_hash(), bincode::serialize(&new_block)?)?;
        self.db.insert("LAST", new_block.get_hash().as_bytes())?;
        self.current_hash = new_block.get_hash();
        Ok(())
    }

    fn find_unspent_transactions(&self, pub_key_hash: &[u8]) -> Vec<Transaction> {
        let mut spent_txos: HashMap<String, Vec<i32>> = HashMap::new();
        let mut unspend_txs: Vec<Transaction> = Vec::new();

        for block in self.iter() {
            for tx in block.get_transaction() {
                for index in 0..tx.vout.len() {
                    if let Some(ids) = spent_txos.get(&tx.id) {
                        if ids.contains(&(index as i32)) {
                            continue;
                        }
                    }

                    if tx.vout[index].is_locked_with_key(pub_key_hash) {
                        unspend_txs.push(tx.to_owned())
                    }
                }

                if !tx.is_coinbase() {
                    for i in &tx.vin {
                        if i.uses_key(pub_key_hash) {
                            match spent_txos.get_mut(&i.txid) {
                                Some(v) => {
                                    v.push(i.vout);
                                }
                                None => {
                                    spent_txos.insert(i.txid.clone(), vec![i.vout]);
                                }
                            }
                        }
                    }
                }
            }
        }
        unspend_txs
    }

    pub fn find_UTXO(&self, pub_key_hash: &[u8]) -> Vec<TXOutput> {
        let mut utxos = Vec::<TXOutput>::new();
        let unspend_txs = self.find_unspent_transactions(pub_key_hash);
        for tx in unspend_txs {
            for out in &tx.vout {
                if out.is_locked_with_key(&pub_key_hash) {
                    utxos.push(out.clone());
                }
            }
        }
        utxos
    }

    pub fn find_spendable_outputs(
        &self,
        pub_key_hash: &[u8],
        amount: i32,
    ) -> (i32, HashMap<String, Vec<i32>>) {
        let mut unspent_outputs: HashMap<String, Vec<i32>> = HashMap::new();
        let mut accumulated = 0;
        let unspend_txs = self.find_unspent_transactions(pub_key_hash);

        for tx in unspend_txs {
            for index in 0..tx.vout.len() {
                if tx.vout[index].is_locked_with_key(pub_key_hash) && accumulated < amount {
                    match unspent_outputs.get_mut(&tx.id) {
                        Some(v) => v.push(index as i32),
                        None => {
                            unspent_outputs.insert(tx.id.clone(), vec![index as i32]);
                        }
                    }
                    accumulated += tx.vout[index].value;

                    if accumulated >= amount {
                        return (accumulated, unspent_outputs);
                    }
                }
            }
        }
        (accumulated, unspent_outputs)
    }

    pub fn iter(&self) -> BlockchainIter {
        BlockchainIter {
            current_hash: self.current_hash.clone(),
            bc: &self,
        }
    }
}

impl<'a> Iterator for BlockchainIter<'a> {
    type Item = Block;

    fn next(&mut self) -> Option<Self::Item> {
        if let Ok(encode_block) = self.bc.db.get(&self.current_hash) {
            return match encode_block {
                Some(b) => {
                    if let Ok(block) = bincode::deserialize::<Block>(&b) {
                        self.current_hash = block.get_prev_hash();
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
