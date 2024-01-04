use std::collections::HashMap;

use anyhow::anyhow;

use crate::block::Block;
use crate::errors::Result;
use crate::transaction::{TXOutput, TXOutputs, Transaction};

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
        db.insert(genesis.get_hash(), bincode::serialize(&genesis)?)
            .expect("Failed to insert");
        db.insert("LAST", genesis.get_hash().as_bytes())?;
        let bc = Blockchain {
            current_hash: genesis.get_hash(),
            db,
        };
        bc.db.flush()?;
        Ok(bc)
    }

    pub fn add_block(&mut self, data: Vec<Transaction>) -> Result<Block> {
        let lasthash = self.db.get("LAST")?.unwrap();

        let new_block = Block::new_block(data, String::from_utf8(lasthash.to_vec())?, TARGET_HEXT)?;
        self.db
            .insert(new_block.get_hash(), bincode::serialize(&new_block)?)?;
        self.db.insert("LAST", new_block.get_hash().as_bytes())?;
        self.current_hash = new_block.get_hash();

        Ok(new_block)
    }

    pub fn find_UTXO(&self) -> HashMap<String, TXOutputs> {
        let mut utxos: HashMap<String, TXOutputs> = HashMap::new();
        let mut unspent_txos: HashMap<String, Vec<i32>> = HashMap::new();

        for block in self.iter() {
            for tx in block.get_transaction() {
                for idx in 0..tx.vout.len() {
                    if let Some(ids) = unspent_txos.get(&tx.id) {
                        if ids.contains(&(idx as i32)) {
                            continue;
                        }
                    }

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

                if !tx.is_coinbase() {
                    for vin in &tx.vin {
                        match unspent_txos.get_mut(&vin.txid) {
                            Some(v) => {
                                v.push(vin.vout);
                            }
                            None => {
                                unspent_txos.insert(vin.txid.clone(), vec![vin.vout]);
                            }
                        }
                    }
                }
            }
        }

        utxos
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
