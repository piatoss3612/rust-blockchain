use crate::{errors::Result, transaction::Transaction};
use anyhow::Ok;
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use log::info;
use std::time::SystemTime;
const TARGET_HEXT: usize = 4;
use merkle_cbt::merkle_tree::Merge;
use merkle_cbt::merkle_tree::CBMT;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct Block {
    timestamp: u128,
    transactions: Vec<Transaction>,
    prev_block_hash: String,
    hash: String,
    height: usize,
    nonce: i32,
}

impl Block {
    pub fn get_transaction(&self) -> &Vec<Transaction> {
        &self.transactions
    }

    pub(crate) fn get_prev_hash(&self) -> String {
        self.prev_block_hash.clone()
    }
    pub fn get_hash(&self) -> String {
        self.hash.clone()
    }

    pub fn new_genesis_block(cbtx: Transaction) -> Block {
        Self::new_block(vec![cbtx], String::new(), 0).unwrap()
    }

    pub fn hash_transactions(&mut self) -> Result<Vec<u8>> {
        let mut transactions = Vec::new();
        for tx in &mut self.transactions {
            transactions.push(tx.hash()?.as_bytes().to_owned());
        }

        let tree = CBMT::<Vec<u8>, MergeTx>::build_merkle_tree(&transactions);

        Ok(tree.root())
    }

    pub fn new_block(
        data: Vec<Transaction>,
        prev_block_hash: String,
        height: usize,
    ) -> Result<Block> {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_millis();
        let mut block = Block {
            timestamp: timestamp,
            transactions: data,
            prev_block_hash,
            hash: String::new(),
            height,
            nonce: 0,
        };
        block.run_proof_if_work()?;
        Ok(block)
    }
    fn run_proof_if_work(&mut self) -> Result<()> {
        info!("Mining the block");
        while !self.validate()? {
            self.nonce += 1;
        }
        let data = self.prepare_hash_data()?;
        let mut hasher = Sha256::new();
        hasher.input(&data[..]);
        self.hash = hasher.result_str();
        Ok(())
    }
    fn prepare_hash_data(&mut self) -> Result<Vec<u8>> {
        let content = (
            self.prev_block_hash.clone(),
            self.hash_transactions()?,
            self.timestamp,
            TARGET_HEXT,
            self.nonce,
        );
        let bytes = bincode::serialize(&content)?;
        Ok(bytes)
    }
    fn validate(&mut self) -> Result<bool> {
        let data = self.prepare_hash_data()?;
        let mut hasher = Sha256::new();
        hasher.input(&data[..]);
        let mut vec1: Vec<u8> = vec![];
        vec1.resize(TARGET_HEXT, '0' as u8);
        Ok(&hasher.result_str()[0..TARGET_HEXT] == String::from_utf8(vec1)?)
    }
}

pub struct MergeTx;

impl Merge for MergeTx {
    type Item = Vec<u8>;

    fn merge(left: &Self::Item, right: &Self::Item) -> Self::Item {
        let mut hasher = Sha256::new();
        let mut data = left.clone();
        data.append(&mut right.clone());
        hasher.input(&data[..]);
        let mut res = [0; 32];
        hasher.result(&mut res);
        res.to_vec()
    }
}
