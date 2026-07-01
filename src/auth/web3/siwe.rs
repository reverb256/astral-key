//! Astral Key - SIWE (Sign-In with Ethereum) verification

use ethers::core::types::{Address, H256, U256};
use std::str::FromStr;

use crate::error::{AuthError, Result};

/// Verify SIWE signature and return the recovered address
pub async fn verify_siwe_signature(
    message: &str,
    signature: &str,
    expected_chain_id: u64,
) -> Result<Address> {
    // Parse SIWE message
    let parsed = parse_siwe_message(message)?;

    // Validate message fields
    validate_siwe_message(&parsed, expected_chain_id)?;

    // Recover address from signature
    let address = recover_address_from_signature(message, signature)?;

    // Verify the address matches the one in the message
    let message_address = Address::from_str(&parsed.address)
        .map_err(|_| AuthError::BadRequest("Invalid address in message".to_string()))?;

    if address != message_address {
        return Err(AuthError::Unauthorized(
            "Signature does not match the address in message".to_string(),
        ));
    }

    Ok(address)
}

/// Parsed SIWE message
#[derive(Debug, Clone)]
struct SiweMessage {
    pub domain: String,
    pub address: String,
    pub statement: Option<String>,
    pub uri: String,
    pub version: String,
    pub chain_id: String,
    pub nonce: String,
    pub issued_at: String,
    pub expiration_time: Option<String>,
    pub not_before: Option<String>,
    pub request_id: Option<String>,
    pub resources: Vec<String>,
}

/// Parse SIWE message from string
fn parse_siwe_message(message: &str) -> Result<SiweMessage> {
    let lines: Vec<&str> = message.lines().collect();

    if lines.len() < 2 {
        return Err(AuthError::BadRequest(
            "Invalid SIWE message format".to_string(),
        ));
    }

    // Extract domain (line 1)
    let domain = lines[0]
        .strip_suffix(" wants you to sign in with your Ethereum account:")
        .ok_or_else(|| AuthError::BadRequest("Invalid SIWE message format".to_string()))?
        .trim()
        .to_string();

    // Extract address (line 2)
    let address = lines[1].trim().to_string();

    // Parse fields
    let mut statement = None;
    let mut uri = String::new();
    let mut version = String::new();
    let mut chain_id = String::new();
    let mut nonce = String::new();
    let mut issued_at = String::new();
    let mut expiration_time = None;
    let mut not_before = None;
    let mut request_id = None;
    let mut resources = Vec::new();
    let mut in_statement = false;
    let mut statement_lines = Vec::new();

    for line in lines.iter().skip(2) {
        let line = line.trim();

        // Collect statement lines (between address and URI)
        if in_statement {
            if line.starts_with("URI:") {
                in_statement = false;
            } else {
                statement_lines.push(line);
                continue;
            }
        }

        if let Some(value) = line.strip_prefix("URI: ") {
            uri = value.to_string();
        } else if let Some(value) = line.strip_prefix("Version: ") {
            version = value.to_string();
        } else if let Some(value) = line.strip_prefix("Chain ID: ") {
            chain_id = value.to_string();
        } else if let Some(value) = line.strip_prefix("Nonce: ") {
            nonce = value.to_string();
        } else if let Some(value) = line.strip_prefix("Issued At: ") {
            issued_at = value.to_string();
        } else if let Some(value) = line.strip_prefix("Expiration Time: ") {
            expiration_time = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("Not Before: ") {
            not_before = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("Request ID: ") {
            request_id = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("Resources:") {
            // Parse resources
            if !value.is_empty() {
                resources = value.split(',').map(|s| s.trim().to_string()).collect();
            }
        } else if line.starts_with("Sign in to") || line.starts_with("Please sign") {
            in_statement = true;
        }
    }

    if !statement_lines.is_empty() {
        statement = Some(statement_lines.join("\n"));
    }

    Ok(SiweMessage {
        domain,
        address,
        statement,
        uri,
        version,
        chain_id,
        nonce,
        issued_at,
        expiration_time,
        not_before,
        request_id,
        resources,
    })
}

/// Validate SIWE message fields
fn validate_siwe_message(message: &SiweMessage, expected_chain_id: u64) -> Result<()> {
    // Validate chain ID
    let message_chain_id: u64 = message
        .chain_id
        .parse()
        .map_err(|_| AuthError::BadRequest("Invalid chain ID in message".to_string()))?;

    if message_chain_id != expected_chain_id {
        return Err(AuthError::BadRequest(format!(
            "Chain ID mismatch: expected {}, got {}",
            expected_chain_id, message_chain_id
        )));
    }

    // Validate version
    if message.version != "1" {
        return Err(AuthError::BadRequest("Invalid SIWE version".to_string()));
    }

    // Validate nonce (should be hex string)
    if message.nonce.is_empty() || message.nonce.len() < 8 {
        return Err(AuthError::BadRequest(
            "Invalid nonce in message".to_string(),
        ));
    }

    // Validate expiration
    if let Some(ref expiration) = message.expiration_time {
        let exp_time = chrono::DateTime::parse_from_rfc3339(expiration)
            .map_err(|_| AuthError::BadRequest("Invalid expiration time format".to_string()))?;

        if exp_time < chrono::Utc::now() {
            return Err(AuthError::Unauthorized("Message has expired".to_string()));
        }
    }

    // Validate not-before if present
    if let Some(ref not_before) = message.not_before {
        let nb_time = chrono::DateTime::parse_from_rfc3339(not_before)
            .map_err(|_| AuthError::BadRequest("Invalid not-before time format".to_string()))?;

        if nb_time > chrono::Utc::now() {
            return Err(AuthError::Unauthorized(
                "Message is not yet valid".to_string(),
            ));
        }
    }

    Ok(())
}

/// Recover Ethereum address from signature
fn recover_address_from_signature(message: &str, signature: &str) -> Result<Address> {
    // Convert signature to bytes
    let sig_bytes = hex::decode(signature.trim_start_matches("0x"))
        .map_err(|_| AuthError::BadRequest("Invalid signature format".to_string()))?;

    if sig_bytes.len() != 65 {
        return Err(AuthError::BadRequest(
            "Invalid signature length: expected 65 bytes".to_string(),
        ));
    }

    // Split signature into r, s, v
    let r = U256::from_big_endian(&sig_bytes[0..32]);
    let s = U256::from_big_endian(&sig_bytes[32..64]);
    let v = sig_bytes[64] as u64;

    // Calculate Ethereum signed message hash
    let message_hash = ethers::core::utils::hash_message(message);

    // Recover address
    let sig = ethers::core::types::Signature { r, s, v };
    let address = sig.recover(message_hash).map_err(|_| {
        AuthError::Unauthorized("Failed to recover address from signature".to_string())
    })?;

    Ok(address)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_siwe_message() {
        let message = "example.com wants you to sign in with your Ethereum account:
0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb

Sign in to Example App

URI: https://example.com
Version: 1
Chain ID: 1
Nonce: abc123
Issued At: 2023-01-01T00:00:00.000Z
Expiration Time: 2023-01-01T01:00:00.000Z";

        let parsed = parse_siwe_message(message).unwrap();
        assert_eq!(parsed.domain, "example.com");
        assert_eq!(parsed.address, "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb");
        assert_eq!(parsed.version, "1");
        assert_eq!(parsed.chain_id, "1");
        assert_eq!(parsed.nonce, "abc123");
    }

    #[test]
    fn test_validate_siwe_message() {
        let message = SiweMessage {
            domain: "example.com".to_string(),
            address: "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string(),
            statement: None,
            uri: "https://example.com".to_string(),
            version: "1".to_string(),
            chain_id: "1".to_string(),
            nonce: "abc123def456".to_string(),
            issued_at: "2023-01-01T00:00:00.000Z".to_string(),
            expiration_time: Some("2099-01-01T00:00:00.000Z".to_string()),
            not_before: None,
            request_id: None,
            resources: Vec::new(),
        };

        assert!(validate_siwe_message(&message, 1).is_ok());
        assert!(validate_siwe_message(&message, 137).is_err()); // Wrong chain ID
    }
}
