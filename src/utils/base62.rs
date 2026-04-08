const ALPHABET: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const BASE: u64 = 62;

/// Encode a u64 into a fixed-length Base62 string (7 characters, zero-padded).
pub fn encode(mut num: u64, length: usize) -> String {
    let mut chars = vec![ALPHABET[0]; length];
    for i in (0..length).rev() {
        chars[i] = ALPHABET[(num % BASE) as usize];
        num /= BASE;
    }
    // Safety: ALPHABET contains only ASCII bytes
    unsafe { String::from_utf8_unchecked(chars) }
}

/// Decode a Base62 string back to u64.
pub fn decode(s: &str) -> Option<u64> {
    let mut result: u64 = 0;
    for &b in s.as_bytes() {
        let digit = match b {
            b'0'..=b'9' => b - b'0',
            b'A'..=b'Z' => b - b'A' + 10,
            b'a'..=b'z' => b - b'a' + 36,
            _ => return None,
        } as u64;
        result = result.checked_mul(BASE)?.checked_add(digit)?;
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_zero() {
        assert_eq!(encode(0, 7), "0000000");
    }

    #[test]
    fn encode_one() {
        assert_eq!(encode(1, 7), "0000001");
    }

    #[test]
    fn roundtrip() {
        for num in [0, 1, 61, 62, 1000, 999_999, 3_521_614_606_207] {
            let encoded = encode(num, 7);
            assert_eq!(encoded.len(), 7);
            assert_eq!(decode(&encoded), Some(num), "roundtrip failed for {num}");
        }
    }

    #[test]
    fn max_7_char_value() {
        // 62^7 - 1 = 3_521_614_606_207
        let max = 3_521_614_606_207u64;
        let encoded = encode(max, 7);
        assert_eq!(encoded, "zzzzzzz");
        assert_eq!(decode(&encoded), Some(max));
    }

    #[test]
    fn decode_invalid_char() {
        assert_eq!(decode("abc!def"), None);
    }

    #[test]
    fn decode_empty() {
        assert_eq!(decode(""), Some(0));
    }
}
