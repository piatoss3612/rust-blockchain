use crate::utils::hash_pub_key;
use crate::utxoset::UTXOSet;
use crate::wallet::Wallets;
use crate::{errors::Result, wallet::Wallet};
use anyhow::anyhow;
use bitcoincash_addr::Address;
use crypto::digest::Digest;
use crypto::ed25519;
use crypto::sha2::Sha256;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Transaction struct that holds the data of the transaction (mimics the Bitcoin transaction)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
    pub id: String,          // Hash of the transaction
    pub vin: Vec<TXInput>,   // Inputs of the transaction
    pub vout: Vec<TXOutput>, // Outputs of the transaction
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TXOutputs {
    pub outputs: Vec<TXOutput>,
}

impl Transaction {
    // Create a new transaction
    // from: the wallet of the sender
    // to: the address of the receiver
    // amount: the amount to be sent
    // utxoset: the UTXO set of from address
    pub fn new_utxo(from: &Wallet, to: &str, amount: i32, utxoset: &UTXOSet) -> Result<Self> {
        // Get the public key hash of the sender
        let mut pub_key_hash = from.public_key.clone();
        hash_pub_key(&mut pub_key_hash);

        // Find the spendable outputs of the sender and the total amount
        let acc_v: (i32, HashMap<String, Vec<i32>>) =
            utxoset.find_spendable_outputs(&pub_key_hash, amount)?;

        // Check if the sender has enough balance
        if acc_v.0 < amount {
            return Err(anyhow!(
                "Not Enough balance for transaction: {} < {}",
                acc_v.0,
                amount
            ));
        }

        // Create the inputs and outputs of the transaction
        let mut vin = Vec::new();

        // Create the inputs of the transaction
        for tx in acc_v.1 {
            // tx.0 is the transaction id
            // tx.1 is the list of output indexes
            for out in tx.1 {
                let input = TXInput {
                    txid: tx.0.clone(),
                    vout: out,
                    signature: Vec::new(),
                    pub_key: from.public_key.clone(),
                };
                vin.push(input);
            }
        }

        // Create the outputs of the transaction
        // vout[0] is for the receiver
        let mut vout = vec![TXOutput::new(amount, to.to_string())?];

        // vout[1] is for the sender (change)
        if acc_v.0 > amount {
            vout.push(TXOutput::new(acc_v.0 - amount, from.get_address())?);
        }

        // Create the transaction
        let mut tx = Transaction {
            id: String::new(),
            vin,
            vout,
        };

        // Set the id of the transaction
        tx.id = tx.hash()?;

        // Sign the transaction with the private key of the sender
        utxoset
            .blockchain
            .sign_transaction(&mut tx, &from.secret_key)?;

        // Return the transaction
        Ok(tx)
    }

    // Create a new coinbase transaction
    // to: the address of the receiver
    // data: the data of the transaction
    pub fn new_coinbase(to: String, mut data: String) -> Result<Self> {
        // If the data is empty, set the default data
        if data.is_empty() {
            data = format!("Reward to '{}'", to);
        }

        // Find the wallet of the receiver
        let wallets = Wallets::new()?;

        if let None = wallets.get_wallet(&to) {
            return Err(anyhow!("wallet not found for address: {}", to));
        }

        // Create the transaction
        // tx.vin[0] is the coinbase input (no previous transaction)
        // tx.vout[0] is for the receiver (the reward)
        let mut tx = Transaction {
            id: String::new(),
            vin: vec![TXInput {
                txid: String::new(),
                vout: -1,
                signature: Vec::new(),
                pub_key: Vec::from(data.as_bytes()),
            }],
            vout: vec![TXOutput::new(100, to)?],
        };

        // Set the id of the transaction
        tx.id = tx.hash()?;

        // Return the transaction
        Ok(tx)
    }

    // Get the transaction id (hash)
    pub(crate) fn hash(&mut self) -> Result<String> {
        // Clear the id of the transaction
        self.id = String::new();

        // Serialize the transaction
        let data = bincode::serialize(self)?;

        // Hash the serialized transaction
        let mut hasher = Sha256::new();
        hasher.input(&data[..]);

        // Return the hash
        Ok(hasher.result_str())
    }
    // Check if the transaction is a coinbase transaction
    pub fn is_coinbase(&self) -> bool {
        // A coinbase transaction has only one input with no previous transaction
        // and the vout is -1
        self.vin.len() == 1 && self.vin[0].txid.is_empty() && self.vin[0].vout == -1
    }

    // Sign the transaction
    // private_key: the private key of the sender
    // prev_txs: has output transactions of the inputs of current transaction
    pub fn sign(
        &mut self,
        private_key: &[u8],
        prev_txs: HashMap<String, Transaction>,
    ) -> Result<()> {
        // If the transaction is a coinbase transaction, return true
        if self.is_coinbase() {
            return Ok(());
        }

        // Check if the previous transactions are correct
        for v in &self.vin {
            match prev_txs.get(&v.txid) {
                Some(prev_tx) => {
                    if prev_tx.id.is_empty() {
                        return Err(anyhow!("previous transaction is not correct"));
                    }
                }
                None => return Err(anyhow!("transaction not found: {}", v.txid)),
            }
        }

        // Create a copy of the current transaction (clear the signature and public key)
        // This copy will be used to generate the signature
        let mut tx_copy = self.trim_copy();

        for idx in 0..tx_copy.vin.len() {
            // Get the previous transaction
            let prev_tx = match prev_txs.get(&tx_copy.vin[idx].txid) {
                Some(prev_tx) => prev_tx,
                None => return Err(anyhow!("transaction not found: {}", tx_copy.vin[idx].txid)),
            };

            // clear the signature of copied transaction
            tx_copy.vin[idx].signature.clear();

            // set the public key hash of previous transaction as public key of copied transaction
            tx_copy.vin[idx].pub_key = prev_tx.vout[tx_copy.vin[idx].vout as usize]
                .pub_key_hash
                .clone();

            // Hash the copied transaction
            tx_copy.id = tx_copy.hash()?;

            // Clear the public key of copied transaction
            tx_copy.vin[idx].pub_key = Vec::new();

            // Generate the signature with the hash of copied transaction and the private key
            let signature = ed25519::signature(tx_copy.id.as_bytes(), private_key);

            // Set the signature of the current transaction
            self.vin[idx].signature = signature.to_vec();
        }

        // Return true if the transaction is signed successfully
        Ok(())
    }

    // Verify the transaction
    // prev_txs: has output transactions of the inputs of current transaction
    pub fn verify(&self, prev_txs: HashMap<String, Transaction>) -> Result<bool> {
        // If the transaction is a coinbase transaction, return true
        if self.is_coinbase() {
            return Ok(true);
        }

        // Check if the previous transactions are correct
        for v in &self.vin {
            match prev_txs.get(&v.txid) {
                Some(prev_tx) => {
                    if prev_tx.id.is_empty() {
                        return Err(anyhow!("previous transaction is not correct"));
                    }
                }
                None => return Err(anyhow!("transaction not found: {}", v.txid)),
            }
        }

        // Create a copy of the current transaction (clear the signature and public key)
        let mut tx_copy = self.trim_copy();

        for idx in 0..self.vin.len() {
            // Get the previous transaction
            let prev_tx = match prev_txs.get(&tx_copy.vin[idx].txid) {
                Some(prev_tx) => prev_tx,
                None => return Err(anyhow!("transaction not found: {}", tx_copy.vin[idx].txid)),
            };

            // Clear the signature of copied transaction
            tx_copy.vin[idx].signature.clear();

            // Set the public key hash of previous transaction as public key of copied transaction
            tx_copy.vin[idx].pub_key = prev_tx.vout[self.vin[idx].vout as usize]
                .pub_key_hash
                .clone();

            // Hash the copied transaction
            tx_copy.id = tx_copy.hash()?;

            // Clear the public key of copied transaction
            tx_copy.vin[idx].pub_key = Vec::new();

            // Verify the signature of the current transaction
            let ok = ed25519::verify(
                &tx_copy.id.as_bytes(),
                &self.vin[idx].pub_key,
                &self.vin[idx].signature,
            );

            // Return false if the signature is not valid
            if !ok {
                return Ok(false);
            }
        }

        // Return true if the transaction is verified successfully
        Ok(true)
    }

    // Create a copy of the current transaction (clear the signature and public key)
    fn trim_copy(&self) -> Self {
        let mut vin = Vec::new();

        // Create the inputs of the copy without the signature and public key
        for v in &self.vin {
            vin.push(TXInput {
                txid: v.txid.clone(),
                vout: v.vout.clone(),
                signature: Vec::new(),
                pub_key: Vec::new(),
            });
        }

        // Return the copy
        Self {
            id: self.id.clone(),
            vin,
            vout: self.vout.clone(),
        }
    }
}

/// TXInput struct for transaction input
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TXInput {
    pub txid: String,       // Transaction id of the previous transaction
    pub vout: i32,          // Index of the output of the previous transaction
    pub signature: Vec<u8>, // Signature of the transaction (signed by the sender)
    pub pub_key: Vec<u8>,   // Public key of the receiver
}

impl TXInput {
    // Check if the public key hash of the input is equal to the public key hash of the sender
    pub fn uses_key(&self, pub_key_hash: &[u8]) -> bool {
        let mut pubkeyhash = self.pub_key.clone();
        hash_pub_key(&mut pubkeyhash);
        pubkeyhash == pub_key_hash
    }
}

/// TXOutput struct for transaction output
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TXOutput {
    pub value: i32,            // Amount of the token to be sent
    pub pub_key_hash: Vec<u8>, // Public key hash of the receiver
}

impl TXOutput {
    // Create a new output
    pub fn new(value: i32, address: String) -> Result<Self> {
        let mut txo = Self {
            value,
            pub_key_hash: Vec::new(),
        };

        // Lock the output with the address of the receiver
        txo.lock(&address)?;

        // Return the output
        Ok(txo)
    }

    // Check if the output is locked with the public key hash
    pub fn is_locked_with_key(&self, pub_key_hash: &[u8]) -> bool {
        self.pub_key_hash == pub_key_hash
    }

    // Lock the output with the address of the receiver
    fn lock(&mut self, address: &str) -> Result<()> {
        // Get the public key hash of the receiver from the address
        let pub_key_hash = Address::decode(address).unwrap().body;
        self.pub_key_hash = pub_key_hash;

        Ok(())
    }
}
