//! ActivityPub types and constants following the W3C ActivityStreams 2.0
//! and ActivityPub specifications.
//!
//! This module defines the types used in federation:
//! - [`Actor`] — ActivityPub actor profile (Person)
//! - [`Activity`] — generic activity wrapper
//! - [`Note`] — a Note object (the most common content type)
//! - [`OrderedCollection`] — paginated collections for inbox/outbox
//! - [`WebFingerResponse`] — WebFinger discovery document
//!
//! All types serialize to JSON-LD with the ActivityStreams context.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ─── Context constants ───────────────────────────────────────────────────────

/// ActivityStreams 2.0 @context URL.
pub const AS_CONTEXT: &str = "https://www.w3.org/ns/activitystreams";

/// W3ID Security vocabulary v1 (for publicKey).
pub const W3ID_SECURITY: &str = "https://w3id.org/security/v1";

// ─── WebFinger ───────────────────────────────────────────────────────────────

/// WebFinger resource descriptor (RFC 7033).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebFingerResponse {
    pub subject: String,
    pub links: Vec<WebFingerLink>,
}

/// A single WebFinger link relation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebFingerLink {
    pub rel: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
}

// ─── Image / Icon ────────────────────────────────────────────────────────────

/// An image object used for actor icon and image properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    #[serde(rename = "type")]
    pub type_: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
}

// ─── Public Key ──────────────────────────────────────────────────────────────

/// Public key block in the Actor profile, used by HTTP Signatures for
/// request verification during federation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKey {
    pub id: String,
    pub owner: String,
    #[serde(rename = "publicKeyPem")]
    pub public_key_pem: String,
}

// ─── Actor ───────────────────────────────────────────────────────────────────

/// An ActivityPub Actor (Person) representing the Mosaic bridge.
///
/// Serves the actor profile at `/actor` and includes inbox, outbox,
/// followers/following endpoints and the Ed25519 public key for
/// HTTP Signature verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Actor {
    #[serde(rename = "@context")]
    pub context: Vec<Value>,
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub inbox: String,
    pub outbox: String,
    pub followers: String,
    pub following: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<Image>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<Image>,
    pub public_key: PublicKey,
}

impl Actor {
    /// Build the bridge's actor profile.
    ///
    /// `domain` is the public domain (e.g. `mosaic.social`).
    /// `public_key_pem` is the PEM-encoded Ed25519 SPKI.
    /// `name` is the optional display name.
    pub fn new(domain: &str, public_key_pem: &str, name: &str) -> Self {
        let base = format!("https://{domain}");
        let actor_id = format!("{base}/actor");
        Self {
            context: vec![
                Value::String(AS_CONTEXT.to_string()),
                serde_json::json!({
                    "security": W3ID_SECURITY,
                    "manuallyApprovesFollowers": "as:manuallyApprovesFollowers",
                    "sensitive": "as:sensitive",
                    "movedTo": "as:movedTo",
                    "Hashtag": "as:Hashtag",
                    "alsoKnownAs": "as:alsoKnownAs",
                    "ostatus": "http://ostatus.org#",
                    "atomUri": "ostatus:atomUri",
                    "inReplyToAtomUri": "ostatus:inReplyToAtomUri",
                    "conversation": "ostatus:conversation",
                    "toot": "http://joinmastodon.org/ns#",
                    "Emoji": "toot:Emoji",
                    "featured": "toot:featured",
                    "discoverable": "toot:discoverable",
                    "schema": "http://schema.org#",
                    "PropertyValue": "schema:PropertyValue",
                    "value": "schema:value",
                    "FocalPoint": "toot:FocalPoint",
                    "vcard": "http://www.w3.org/2006/vcard/ns#",
                }),
            ],
            type_: "Person".to_string(),
            id: actor_id.clone(),
            name: Some(name.to_string()),
            preferred_username: Some("mosaic".to_string()),
            summary: Some(format!(
                "<p>Mosaic Identity Bridge — ActivityPub federation gateway.</p>",
            )),
            inbox: format!("{base}/inbox"),
            outbox: format!("{base}/outbox"),
            followers: format!("{base}/followers"),
            following: format!("{base}/following"),
            icon: None,
            image: None,
            public_key: PublicKey {
                id: format!("{actor_id}#main-key"),
                owner: actor_id,
                public_key_pem: public_key_pem.to_string(),
            },
        }
    }
}

// ─── Activity ────────────────────────────────────────────────────────────────

/// A generic ActivityStreams Activity.
///
/// Used for incoming and outgoing activities (Follow, Create, Like, Announce,
/// Undo, Delete, Accept, Reject).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity {
    #[serde(rename = "@context", skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
    #[serde(default)]
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub actor: Value,
    #[serde(default)]
    pub object: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cc: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published: Option<String>,
}

// ─── Note ────────────────────────────────────────────────────────────────────

/// A Note object — the primary content type for messages posted through
/// the ActivityPub bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    #[serde(rename = "@context", skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
    #[serde(default)]
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub attributed_to: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub published: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cc: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<Value>,
}

// ─── Collections ─────────────────────────────────────────────────────────────

/// An OrderdCollection used for inbox and outbox endpoints per the
/// ActivityPub specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderedCollection {
    #[serde(rename = "@context", skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_items: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ordered_items: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<Value>,
}

impl OrderedCollection {
    /// Create a new empty ordered collection.
    pub fn new(id: String) -> Self {
        Self {
            context: Some(Value::String(AS_CONTEXT.to_string())),
            id,
            type_: "OrderedCollection".to_string(),
            total_items: Some(0),
            ordered_items: Some(Vec::new()),
            first: None,
            last: None,
        }
    }

    /// Create an ordered collection with items.
    pub fn with_items(id: String, items: Vec<Value>) -> Self {
        let count = items.len();
        Self {
            context: Some(Value::String(AS_CONTEXT.to_string())),
            id,
            type_: "OrderedCollection".to_string(),
            total_items: Some(count),
            ordered_items: Some(items),
            first: None,
            last: None,
        }
    }
}

/// A flat Collection (used for followers, following).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    #[serde(rename = "@context", skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_items: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first: Option<Value>,
}

impl Collection {
    /// Create a new collection.
    pub fn new(id: String) -> Self {
        Self {
            context: Some(Value::String(AS_CONTEXT.to_string())),
            id,
            type_: "Collection".to_string(),
            total_items: Some(0),
            items: Some(Vec::new()),
            first: None,
        }
    }

    /// Create a collection with items.
    pub fn with_items(id: String, items: Vec<Value>) -> Self {
        let count = items.len();
        Self {
            context: Some(Value::String(AS_CONTEXT.to_string())),
            id,
            type_: "Collection".to_string(),
            total_items: Some(count),
            items: Some(items),
            first: None,
        }
    }
}

// ─── Activity type constants ─────────────────────────────────────────────────

/// Activity type constants used throughout the bridge.
pub mod activity_types {
    pub const FOLLOW: &str = "Follow";
    pub const ACCEPT: &str = "Accept";
    pub const REJECT: &str = "Reject";
    pub const CREATE: &str = "Create";
    pub const DELETE: &str = "Delete";
    pub const UPDATE: &str = "Update";
    pub const LIKE: &str = "Like";
    pub const ANNOUNCE: &str = "Announce";
    pub const UNDO: &str = "Undo";
}

// ─── Helper utilities ────────────────────────────────────────────────────────

/// Generate a unique IRI for an activity.
pub fn activity_id(domain: &str, uuid: &str) -> String {
    format!("https://{domain}/activities/{uuid}")
}

/// Generate a unique IRI for a Note object.
pub fn note_id(domain: &str, uuid: &str) -> String {
    format!("https://{domain}/notes/{uuid}")
}
