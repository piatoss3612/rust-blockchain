use crypto::{digest::Digest, ripemd160::Ripemd160, sha2::Sha256};

// Hashes a public key using SHA256 and RIPEMD160
pub fn hash_pub_key(pub_key: &mut Vec<u8>) {
    // Hash the public key using SHA256
    let mut hasher1 = Sha256::new();
    hasher1.input(pub_key);
    hasher1.result(pub_key);

    // Hash the public key using RIPEMD160
    let mut hasher2 = Ripemd160::new();
    hasher2.input(pub_key);
    pub_key.resize(20, 0);
    hasher2.result(pub_key);
}
