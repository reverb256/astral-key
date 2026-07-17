use bech32::decode;
use serde::{Deserialize, Serialize};
use crate::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedNostrIdentity {
    pub npub: String,
    pub hex_pubkey: String,
    pub mosaic: ResolvedNostrMosaic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedNostrMosaic {
    pub external_id: String,
    pub display_name: String,
}

/// Validate + decode an npub to its hex public key.
/// Returns the original npub on success.
pub fn resolve_npub(input: &str) -> Result<ResolvedNostrIdentity, Error> {
    let clean = input.trim().trim_start_matches('@').trim_start_matches("nostr:");

    let (hrp, data) = decode(clean).map_err(|e| {
        Error::BadRequest(format!("Invalid bech32 '{}': {}", clean, e))
    })?;

    if hrp.as_str() != "npub" {
        return Err(Error::BadRequest(format!("Expected 'npub' prefix, got '{}'", hrp)));
    }
    if data.len() != 32 {
        return Err(Error::BadRequest(format!("npub must be 32 bytes, got {}", data.len())));
    }

    let hex_pubkey = hex::encode(&data);

    Ok(ResolvedNostrIdentity {
        npub: clean.to_string(),
        hex_pubkey: hex_pubkey.clone(),
        mosaic: ResolvedNostrMosaic {
            external_id: hex_pubkey,
            display_name: format!("nostr:{}", &clean.chars().take(16).collect::<String>()),
        },
    })
}
