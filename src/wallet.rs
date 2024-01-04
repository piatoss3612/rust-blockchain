use crate::{errors::Result, utils::hash_pub_key};
use bitcoincash_addr::{Address, HashType, Scheme};
use crypto::ed25519;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Wallet struct contains secret_key and public_key of ed25519
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Wallet {
    pub secret_key: Vec<u8>,
    pub public_key: Vec<u8>,
}

impl Wallet {
    // Create a new wallet
    fn new() -> Self {
        // Generate a random 32 bytes key
        let mut key: [u8; 32] = [0; 32];
        OsRng.fill_bytes(&mut key);

        // Generate a pair of secret_key and public_key
        let (secrect_key, public_key) = ed25519::keypair(&key);

        let secret_key = secrect_key.to_vec();
        let public_key = public_key.to_vec();

        // Return a new wallet
        Self {
            secret_key,
            public_key,
        }
    }

    // Get address from public_key
    fn get_address(&self) -> String {
        // Hash public_key
        let mut pub_hash = self.public_key.clone();
        hash_pub_key(&mut pub_hash);

        // Encode address (base58 encoding)
        let address = Address {
            body: pub_hash,
            scheme: Scheme::Base58,
            hash_type: HashType::Script,
            ..Default::default()
        };

        // Return address
        address.encode().unwrap()
    }
}

// Wallets struct contains a HashMap of Wallet
pub struct Wallets {
    wallets: HashMap<String, Wallet>, // address -> wallet mapping
}

impl Wallets {
    // Create a new Wallets
    pub fn new() -> Result<Self> {
        // Create a new Wallets
        let mut w: Wallets = Self {
            wallets: HashMap::<String, Wallet>::new(),
        };

        // Load wallets from database
        let db = sled::open("data/wallets")?;

        for item in db.into_iter() {
            let i = item?;
            let address = String::from_utf8(i.0.to_vec())?;
            let wallet = bincode::deserialize(&i.1.to_vec())?;
            w.wallets.insert(address, wallet);
        }

        // Drop database
        drop(db);

        // Return a new Wallets
        Ok(w)
    }

    // Create a new wallet and return its address
    pub fn create_wallet(&mut self) -> String {
        // Create a new wallet
        let wallet = Wallet::new();
        let address = wallet.get_address();

        // Insert the wallet into wallets
        self.wallets.insert(address.clone(), wallet);

        // Return the address
        address
    }

    // Get all addresses in wallets
    pub fn get_all_address(&self) -> Vec<String> {
        let mut addresses = Vec::new();

        for (address, _) in &self.wallets {
            addresses.push(address.clone())
        }

        // Return all addresses
        addresses
    }

    // Get wallet by address
    pub fn get_wallet(&self, address: &str) -> Option<&Wallet> {
        self.wallets.get(address)
    }

    // Save all wallets into database
    pub fn save_all(&self) -> Result<()> {
        let db = sled::open("data/wallets")?;

        for (address, wallet) in &self.wallets {
            let data = bincode::serialize(wallet)?;
            db.insert(address, data)?;
        }

        // Flush and drop database
        db.flush()?;
        drop(db);

        Ok(())
    }
}
