use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::{
    aead::{Aead, OsRng},
    Aes256Gcm, Nonce,
};

use sha2::{Digest, Sha256};

pub const SALT: &[u8] = b"Ascendustry_v1_LE48TRUC48DE48FOU";
pub const NONCE_LEN: usize = 12;

pub fn generate_server_id() -> [u8; 16] {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).expect("Failed to generate random bytes");
    bytes
}

pub fn server_id_to_hex(id: &[u8]) -> String {
    id.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn compute_shared_secret(server_id: &[u8], token: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(server_id);
    hasher.update(SALT);
    hasher.update(token);
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

pub fn aes_encrypt(data: &[u8], cipher: &Aes256Gcm) -> Result<Vec<u8>, aes_gcm::Error> {
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let mut payload = cipher.encrypt(nonce, data)?;
    let mut output = nonce_bytes.to_vec();
    output.append(&mut payload);

    Ok(output)
}

pub fn aes_decrypt(data: &[u8], cipher: &Aes256Gcm) -> Result<Vec<u8>, aes_gcm::Error> {
    if data.len() < NONCE_LEN {
        return Err(aes_gcm::Error);
    }

    let (nonce_bytes, ciphertext) = data.split_at(NONCE_LEN);
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher.decrypt(nonce, ciphertext)
}

pub fn xor_crypt(data: &[u8], key: &[u8]) -> Vec<u8> {
    data.iter().enumerate().map(|(i, &b)| b ^ key[i % key.len()]).collect()
}
