use crate::{errors::Result, transaction::Transaction};
use anyhow::Ok;
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use merkle_cbt::merkle_tree::Merge;
use merkle_cbt::merkle_tree::CBMT;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

const TARGET_HEXT: usize = 4; // difficulty of the mining

// Block struct that holds the data of the block
#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct Block {
    timestamp: u128, // Time of the block creation in milliseconds since the Unix Epoch
    transactions: Vec<Transaction>, // Transactions that are included in the block
    prev_block_hash: String, // Hash of the previous block
    hash: String,    // Hash of the block
    height: usize,   // Height of the block in the blockchain
    nonce: i32,      // Nonce of the block
}

impl Block {
    // Getters for the block struct
    pub fn get_transactions(&self) -> &Vec<Transaction> {
        &self.transactions
    }

    pub(crate) fn get_prev_hash(&self) -> String {
        self.prev_block_hash.clone()
    }
    pub fn get_hash(&self) -> String {
        self.hash.clone()
    }

    // =========================================

    /// Create a genesis block
    pub fn new_genesis_block(cbtx: Transaction) -> Self {
        Self::new_block(vec![cbtx], String::new(), 0).unwrap()
    }

    // Create a new block
    // data: Transactions that are included in the block
    // prev_block_hash: Hash of the previous block
    // height: Height of the block in the blockchain
    pub fn new_block(
        data: Vec<Transaction>,
        prev_block_hash: String,
        height: usize,
    ) -> Result<Self> {
        // Get the current time in milliseconds since the Unix Epoch
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_millis();

        // Create a new block
        let mut block = Self {
            timestamp,
            transactions: data,
            prev_block_hash,
            hash: String::new(),
            height,
            nonce: 0, // Set the nonce to 0 for now
        };

        // Run the dummy proof of work algorithm to get the hash of the block
        block.run_proof_if_work()?;

        // Return the block
        Ok(block)
    }

    // Run the dummy proof of work algorithm
    fn run_proof_if_work(&mut self) -> Result<()> {
        // Loop until the block is valid
        while !self.validate()? {
            self.nonce += 1;
        }

        // Get the hash of the block
        let data = self.serialize_block()?;
        let mut hasher = Sha256::new();
        hasher.input(&data[..]);
        self.hash = hasher.result_str();

        // Done mining
        Ok(())
    }

    // Hash the data of the block
    fn serialize_block(&mut self) -> Result<Vec<u8>> {
        // Serialize the block
        let content = (
            self.prev_block_hash.clone(),
            self.hash_transactions()?,
            self.timestamp,
            TARGET_HEXT,
            self.nonce,
        );
        let bytes = bincode::serialize(&content)?;

        // Return the serialized block
        Ok(bytes)
    }

    // Create merkle tree of the transactions and return the root hash
    pub fn hash_transactions(&mut self) -> Result<Vec<u8>> {
        let mut transactions = Vec::new();

        // Get the hash of each transaction and push it to the transactions vector
        for tx in &mut self.transactions {
            transactions.push(tx.hash()?.as_bytes().to_vec());
        }

        // Create a merkle tree from the transactions
        let tree = CBMT::<Vec<u8>, MergeTx>::build_merkle_tree(&transactions);

        // Return the root hash of the merkle tree
        Ok(tree.root())
    }

    // Validate the block
    fn validate(&mut self) -> Result<bool> {
        // Get the hash of the block
        let data = self.serialize_block()?;
        let mut hasher = Sha256::new();
        hasher.input(&data[..]);

        // Check if the prefix of the hash includes 'TARGET_HEXT' zeros (difficulty)
        let mut target: Vec<u8> = vec![];
        target.resize(TARGET_HEXT, '0' as u8);

        Ok(hasher.result_str()[0..TARGET_HEXT] == String::from_utf8(target)?)
    }
}

// Implement the merge trait for the merkle tree
pub struct MergeTx;

impl Merge for MergeTx {
    type Item = Vec<u8>; // The type of the data that is stored in the merkle tree

    // Merge two hashes
    // left: Hash of the left node
    // right: Hash of the right node
    fn merge(left: &Self::Item, right: &Self::Item) -> Self::Item {
        let mut hasher = Sha256::new();
        let mut data = left.clone();

        // Concatenate the two hashes and hash them
        data.append(&mut right.clone());
        hasher.input(&data[..]);

        let mut res = [0; 32];
        hasher.result(&mut res);

        // Return the hash
        res.to_vec()
    }
}
