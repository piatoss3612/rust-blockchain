use std::collections::HashMap;

use crate::blockchain::Blockchain;
use crate::errors::Result;
use crate::utxoset::UTXOSet;
use crate::wallet::Wallets;
use anyhow::anyhow;
use bitcoincash_addr::Address;
use crypto::ed25519;
use crypto::sha2::Sha256;
use crypto::{digest::Digest, ripemd160::Ripemd160};
use serde::{Deserialize, Serialize};

/// Transaction represents a Bitcoin transaction
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
    pub id: String,
    pub vin: Vec<TXInput>,
    pub vout: Vec<TXOutput>,
}

impl Transaction {
    /// NewUTXOTransaction creates a new transaction
    pub fn new_UTXO(from: &str, to: &str, amount: i32, utxoset: &UTXOSet) -> Result<Transaction> {
        let mut vin = Vec::new();

        let wallets = Wallets::new()?;

        let wallet = match wallets.get_wallet(from) {
            Some(w) => w,
            None => return Err(anyhow!("No wallet found for address: {}", from)),
        };

        if let None = wallets.get_wallet(to) {
            return Err(anyhow!("No wallet found for address: {}", to));
        }

        let mut pub_key_hash = wallet.public_key.clone();
        hash_pub_key(&mut pub_key_hash);

        let acc_v = utxoset.find_spendable_outputs(&pub_key_hash, amount)?;

        if acc_v.0 < amount {
            return Err(anyhow!(
                "Not Enough balance for transaction: {} < {}",
                acc_v.0,
                amount
            ));
        }

        for tx in acc_v.1 {
            for out in tx.1 {
                let input = TXInput {
                    txid: tx.0.clone(),
                    vout: out,
                    signature: Vec::new(),
                    pub_key: wallet.public_key.clone(),
                };
                vin.push(input);
            }
        }

        let mut vout = vec![TXOutput::new(amount, to.to_string())?];

        if acc_v.0 > amount {
            vout.push(TXOutput::new(acc_v.0 - amount, from.to_string())?);
        }

        let mut tx = Transaction {
            id: String::new(),
            vin,
            vout,
        };

        tx.id = tx.hash()?;
        utxoset
            .blockchain
            .sign_transaction(&mut tx, &wallet.secret_key)?;
        Ok(tx)
    }

    pub fn new_coinbase(to: String, mut data: String) -> Result<Transaction> {
        if data == String::from("") {
            data += &format!("Reward to '{}'", to);
        }

        let walltes = Wallets::new()?;
        if let None = walltes.get_wallet(&to) {
            return Err(anyhow!("coinbase wallet not found"));
        }

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
        tx.id = tx.hash()?;
        Ok(tx)
    }

    /// SetID sets ID of a transaction
    fn hash(&mut self) -> Result<String> {
        self.id = String::new();
        let data = bincode::serialize(self)?;
        let mut hasher = Sha256::new();
        hasher.input(&data[..]);
        Ok(hasher.result_str())
    }
    /// IsCoinbase checks whether the transaction is coinbase
    pub fn is_coinbase(&self) -> bool {
        self.vin.len() == 1 && self.vin[0].txid.is_empty() && self.vin[0].vout == -1
    }

    pub fn verify(&mut self, prev_txs: HashMap<String, Transaction>) -> Result<bool> {
        if self.is_coinbase() {
            return Ok(true);
        }

        for vin in &self.vin {
            if prev_txs.get(&vin.txid).unwrap().id.is_empty() {
                return Err(anyhow!("ERROR: Previous transaction is not correct"));
            }
        }
        let mut tx_copy = self.trim_copy();

        for in_id in 0..self.vin.len() {
            let prev_tx = prev_txs.get(&self.vin[in_id].txid).unwrap();
            tx_copy.vin[in_id].signature.clear();
            tx_copy.vin[in_id].pub_key = prev_tx.vout[self.vin[in_id].vout as usize]
                .pub_key_hash
                .clone();
            tx_copy.id = tx_copy.hash()?;
            tx_copy.vin[in_id].pub_key = Vec::new();

            if !ed25519::verify(
                &tx_copy.id.as_bytes(),
                &self.vin[in_id].pub_key,
                &self.vin[in_id].signature,
            ) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    pub fn sign(
        &mut self,
        private_key: &[u8],
        prev_txs: HashMap<String, Transaction>,
    ) -> Result<()> {
        if self.is_coinbase() {
            return Ok(());
        }
        for vin in &self.vin {
            if prev_txs.get(&vin.txid).unwrap().id.is_empty() {
                return Err(anyhow!("ERROR: Previous transaction is not correct"));
            }
        }

        let mut tx_copy = self.trim_copy();

        for in_id in 0..tx_copy.vin.len() {
            let prev_tx = prev_txs.get(&tx_copy.vin[in_id].txid).unwrap();
            tx_copy.vin[in_id].signature.clear();
            tx_copy.vin[in_id].pub_key = prev_tx.vout[tx_copy.vin[in_id].vout as usize]
                .pub_key_hash
                .clone();
            tx_copy.id = tx_copy.hash()?;
            tx_copy.vin[in_id].pub_key = Vec::new();
            let signature = ed25519::signature(tx_copy.id.as_bytes(), private_key);
            self.vin[in_id].signature = signature.to_vec();
        }

        Ok(())
    }
    fn trim_copy(&self) -> Transaction {
        let mut vin = Vec::new();
        let mut vout = Vec::new();

        for v in &self.vin {
            vin.push(TXInput {
                txid: v.txid.clone(),
                vout: v.vout.clone(),
                signature: Vec::new(),
                pub_key: Vec::new(),
            });
        }

        for v in &self.vout {
            vout.push(TXOutput {
                value: v.value,
                pub_key_hash: v.pub_key_hash.clone(),
            });
        }

        Transaction {
            id: self.id.clone(),
            vin,
            vout,
        }
    }
}

/// TXInput represents a transaction input
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TXInput {
    pub txid: String,
    pub vout: i32,
    pub signature: Vec<u8>,
    pub pub_key: Vec<u8>,
}

impl TXInput {
    pub fn uses_key(&self, pub_key_hash: &[u8]) -> bool {
        let mut pubkeyhash = self.pub_key.clone();
        hash_pub_key(&mut pubkeyhash);
        pubkeyhash == pub_key_hash
    }
}

/// TXOutput represents a transaction output
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TXOutput {
    pub value: i32,
    pub pub_key_hash: Vec<u8>,
}

impl TXOutput {
    pub fn is_locked_with_key(&self, pub_key_hash: &[u8]) -> bool {
        self.pub_key_hash == pub_key_hash
    }

    pub fn new(value: i32, address: String) -> Result<Self> {
        let mut txo = Self {
            value,
            pub_key_hash: Vec::new(),
        };

        txo.lock(&address)?;

        Ok(txo)
    }

    pub fn lock(&mut self, address: &str) -> Result<()> {
        let pub_key_hash = Address::decode(address).unwrap().body;
        self.pub_key_hash = pub_key_hash;
        Ok(())
    }
}

pub fn hash_pub_key(pub_key: &mut Vec<u8>) {
    let mut hasher1 = Sha256::new();
    hasher1.input(pub_key);
    hasher1.result(pub_key);
    let mut hasher2 = Ripemd160::new();
    hasher2.input(pub_key);
    pub_key.resize(20, 0);
    hasher2.result(pub_key);
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TXOutputs {
    pub outputs: Vec<TXOutput>,
}
