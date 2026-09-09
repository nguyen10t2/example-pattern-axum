use rand::Rng;

/// Generates a cryptographically secure 6-digit OTP (100000 - 999999).
#[must_use]
pub fn generate_otp() -> String {
    let mut rng = rand::thread_rng();
    let num: u32 = rng.gen_range(100_000..1_000_000);
    num.to_string()
}

/// Generates a cryptographically secure 8-character hex invite code.
#[must_use]
pub fn generate_invite_code() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 4] = rng.r#gen();
    hex::encode(bytes).to_uppercase()
}

mod hex {
    /// Encode 4 bytes thành hex lowercase.
    pub(super) fn encode(data: [u8; 4]) -> String {
        format!("{:02x}{:02x}{:02x}{:02x}", data[0], data[1], data[2], data[3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_otp() {
        let otp = generate_otp();
        assert_eq!(otp.len(), 6);
        let num: u32 = otp.parse().unwrap();
        assert!((100_000..=999_999).contains(&num));
    }

    #[test]
    fn test_generate_invite_code() {
        let code = generate_invite_code();
        assert_eq!(code.len(), 8);
        assert!(code.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
