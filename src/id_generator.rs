use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::utils::base62;

const SHORT_CODE_LENGTH: usize = 7;
const HALF_BITS: u32 = 21;
const HALF_MASK: u64 = (1 << HALF_BITS) - 1; // 0x1FFFFF
const FEISTEL_ROUNDS: usize = 4;
const MAX_VALUE: u64 = 3_521_614_606_207; // 62^7 - 1

/// Trait for allocating sequential IDs from Redis.
#[async_trait]
pub trait IdAllocator: Send + Sync {
    /// Allocate a batch of IDs. Returns (start, end) inclusive range.
    async fn allocate_batch(&self, batch_size: u64) -> Result<(u64, u64), IdGenError>;
}

#[derive(Debug, thiserror::Error)]
pub enum IdGenError {
    #[error("id allocator error: {0}")]
    Alloc(String),
    #[error("id space exhausted")]
    Exhausted,
}

/// Generates short codes using Redis batch allocation + Feistel network + Base62.
pub struct IdGenerator {
    allocator: Arc<dyn IdAllocator>,
    batch_size: u64,
    round_keys: [u64; FEISTEL_ROUNDS],
    state: Mutex<BatchState>,
}

struct BatchState {
    current: u64,
    end: u64,
}

impl IdGenerator {
    pub fn new(allocator: Arc<dyn IdAllocator>, batch_size: u64, master_key: u64) -> Self {
        let round_keys = derive_round_keys(master_key);
        Self {
            allocator,
            batch_size,
            round_keys,
            state: Mutex::new(BatchState { current: 0, end: 0 }),
        }
    }

    /// Generate the next short code.
    pub async fn next_short_code(&self) -> Result<String, IdGenError> {
        let id = self.next_id().await?;
        let obfuscated = self.feistel_encrypt(id);
        Ok(base62::encode(obfuscated, SHORT_CODE_LENGTH))
    }

    async fn next_id(&self) -> Result<u64, IdGenError> {
        // Fast path: try to get next ID from current batch (lock held briefly)
        {
            let mut state = self.state.lock().unwrap();
            if state.current < state.end {
                let id = state.current;
                state.current += 1;
                if id > MAX_VALUE {
                    return Err(IdGenError::Exhausted);
                }
                return Ok(id);
            }
        }
        // Batch exhausted — allocate new batch (no lock held during async call)
        let (start, end) = self.allocator.allocate_batch(self.batch_size).await?;
        let mut state = self.state.lock().unwrap();
        // Another task might have allocated while we were awaiting — check again
        if state.current >= state.end {
            state.current = start;
            state.end = end;
        }
        let id = state.current;
        state.current += 1;
        if id > MAX_VALUE {
            return Err(IdGenError::Exhausted);
        }
        Ok(id)
    }

    /// Feistel network: bijection on 42-bit space with cycle walking.
    fn feistel_encrypt(&self, mut value: u64) -> u64 {
        loop {
            let encrypted = self.feistel_round(value);
            if encrypted <= MAX_VALUE {
                return encrypted;
            }
            // Cycle walking: re-encrypt until result is in valid range
            value = encrypted;
        }
    }

    fn feistel_round(&self, value: u64) -> u64 {
        let mut left = (value >> HALF_BITS) & HALF_MASK;
        let mut right = value & HALF_MASK;

        for key in &self.round_keys {
            let new_left = right;
            let f = round_function(right, *key);
            right = left ^ f;
            left = new_left;
        }

        (left << HALF_BITS) | right
    }

    /// Feistel decrypt (reverse rounds) — for testing bijection.
    #[cfg(test)]
    fn feistel_decrypt(&self, value: u64) -> u64 {
        let mut left = (value >> HALF_BITS) & HALF_MASK;
        let mut right = value & HALF_MASK;

        for key in self.round_keys.iter().rev() {
            let new_right = left;
            let f = round_function(left, *key);
            left = right ^ f;
            right = new_right;
        }

        (left << HALF_BITS) | right
    }
}

fn round_function(half: u64, key: u64) -> u64 {
    ((half.wrapping_mul(key)) ^ (half >> 7)) & HALF_MASK
}

/// Derive round keys from master key using splitmix64-like hash.
fn derive_round_keys(mut seed: u64) -> [u64; FEISTEL_ROUNDS] {
    let mut keys = [0u64; FEISTEL_ROUNDS];
    for key in &mut keys {
        seed = seed.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^= z >> 31;
        *key = z;
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn feistel_bijection() {
        let id_gen = IdGenerator::new(Arc::new(DummyAllocator), 1000, 0xDEAD_BEEF_CAFE_BABE);
        for id in 0..1000 {
            let encrypted = id_gen.feistel_encrypt(id);
            assert!(encrypted <= MAX_VALUE, "encrypted {encrypted} out of range");
            let mut decrypted = encrypted;
            loop {
                decrypted = id_gen.feistel_decrypt(decrypted);
                if decrypted <= MAX_VALUE {
                    break;
                }
            }
            assert_eq!(decrypted, id, "bijection failed for {id}");
        }
    }

    #[test]
    fn feistel_no_collisions() {
        let id_gen = IdGenerator::new(Arc::new(DummyAllocator), 1000, 0xDEAD_BEEF_CAFE_BABE);
        let mut seen = HashSet::new();
        for id in 0..10_000 {
            let encrypted = id_gen.feistel_encrypt(id);
            assert!(seen.insert(encrypted), "collision at id {id}");
        }
    }

    #[test]
    fn feistel_looks_random() {
        let id_gen = IdGenerator::new(Arc::new(DummyAllocator), 1000, 0xDEAD_BEEF_CAFE_BABE);
        let a = id_gen.feistel_encrypt(0);
        let b = id_gen.feistel_encrypt(1);
        let c = id_gen.feistel_encrypt(2);
        // Sequential inputs should NOT produce sequential outputs
        assert_ne!(b.abs_diff(a), 1, "outputs look sequential");
        assert_ne!(c.abs_diff(b), 1, "outputs look sequential");
    }

    struct DummyAllocator;

    #[async_trait]
    impl IdAllocator for DummyAllocator {
        async fn allocate_batch(&self, _batch_size: u64) -> Result<(u64, u64), IdGenError> {
            Ok((0, 1_000_000))
        }
    }
}
