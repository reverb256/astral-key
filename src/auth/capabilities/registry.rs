//! Known scopes for astral-key capability tokens.
//!
//! This is the compile-time registry of all valid scopes.
//! Scopes not in this list are rejected at mint time.
//!
//! ## Convention
//!
//! - All scopes use lowercase ASCII
//! - Format: `namespace:action` (or deeper nesting)
//! - The wildcard `"admin"` grants all scopes
//!
//! When adding a new scope, add it to both [`is_known_scope`] and
//! [`known_scopes`] to keep the registry consistent.

/// Top-level scope namespaces defined by the astral-key capability model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Namespace {
    /// Authentication operations (passkey, web3, token management)
    Auth,
    /// API key management
    Key,
    /// JIT capability token operations
    Jit,
    /// MCP tool and resource access
    Mcp,
    /// DNS record management (homelab-specific, opt-in)
    Dns,
    /// Pages deployment (homelab-specific, opt-in)
    Pages,
}

impl Namespace {
    /// Return the string prefix for this namespace.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Auth => "auth",
            Self::Key => "key",
            Self::Jit => "jit",
            Self::Mcp => "mcp",
            Self::Dns => "dns",
            Self::Pages => "pages",
        }
    }

    /// Parse a namespace from a string prefix.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "auth" => Some(Self::Auth),
            "key" => Some(Self::Key),
            "jit" => Some(Self::Jit),
            "mcp" => Some(Self::Mcp),
            "dns" => Some(Self::Dns),
            "pages" => Some(Self::Pages),
            _ => None,
        }
    }
}

/// Check whether a scope string is a known, registered scope.
///
/// Returns `true` if the scope is in the compile-time allowlist or is
/// the `"admin"` wildcard.
///
/// ## Examples
///
/// ```ignore
/// assert!(is_known_scope("dns:read"));
/// assert!(is_known_scope("admin"));
/// assert!(is_known_scope("mcp:tools:call:read"));
///
/// assert!(!is_known_scope("unknown:scope"));
/// assert!(!is_known_scope("dns:delete"));   // Not registered
/// ```
pub fn is_known_scope(scope: &str) -> bool {
    matches!(
        scope,
        // Auth scopes
        "auth:passkey:register" |
        "auth:passkey:authenticate" |
        "auth:web3:authenticate" |
        "auth:token:refresh" |
        "auth:token:revoke" |
        // API key scopes
        "key:create" |
        "key:read" |
        "key:revoke" |
        // JIT scopes
        "jit:mint" |
        "jit:verify" |
        // MCP scopes
        "mcp:tools:list" |
        "mcp:tools:call:read" |
        "mcp:tools:call:write" |
        "mcp:resources:read" |
        // Resource scopes (homelab-specific, opt-in)
        "dns:read" |
        "dns:write" |
        "pages:deploy" |
        "pages:read" |
        // Wildcard admin
        "admin"
    )
}

/// Return the complete list of known scope strings.
///
/// This is the source of truth for all valid scopes. The issuer should
/// call this to validate requested scopes before minting a token.
pub fn known_scopes() -> Vec<&'static str> {
    vec![
        // Auth scopes
        "auth:passkey:register",
        "auth:passkey:authenticate",
        "auth:web3:authenticate",
        "auth:token:refresh",
        "auth:token:revoke",
        // API key scopes
        "key:create",
        "key:read",
        "key:revoke",
        // JIT scopes
        "jit:mint",
        "jit:verify",
        // MCP scopes
        "mcp:tools:list",
        "mcp:tools:call:read",
        "mcp:tools:call:write",
        "mcp:resources:read",
        // Resource scopes (homelab-specific, opt-in)
        "dns:read",
        "dns:write",
        "pages:deploy",
        "pages:read",
        // Wildcard admin
        "admin",
    ]
}

/// Get the namespace prefix for a scope string.
///
/// Returns `None` for scopes that don't start with a known namespace
/// or for the `"admin"` wildcard.
///
/// ## Examples
///
/// ```ignore
/// assert_eq!(namespace_of("dns:write"), Some(Namespace::Dns));
/// assert_eq!(namespace_of("admin"), None);
/// assert_eq!(namespace_of("unknown:scope"), None);
/// ```
pub fn namespace_of(scope: &str) -> Option<Namespace> {
    if scope == "admin" {
        return None;
    }
    let first_colon = scope.find(':')?;
    Namespace::from_str(&scope[..first_colon])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_known_scope_valid() {
        assert!(is_known_scope("admin"));
        assert!(is_known_scope("dns:read"));
        assert!(is_known_scope("dns:write"));
        assert!(is_known_scope("pages:deploy"));
        assert!(is_known_scope("pages:read"));
        assert!(is_known_scope("auth:passkey:register"));
        assert!(is_known_scope("auth:passkey:authenticate"));
        assert!(is_known_scope("auth:web3:authenticate"));
        assert!(is_known_scope("auth:token:refresh"));
        assert!(is_known_scope("auth:token:revoke"));
        assert!(is_known_scope("key:create"));
        assert!(is_known_scope("key:read"));
        assert!(is_known_scope("key:revoke"));
        assert!(is_known_scope("jit:mint"));
        assert!(is_known_scope("jit:verify"));
        assert!(is_known_scope("mcp:tools:list"));
        assert!(is_known_scope("mcp:tools:call:read"));
        assert!(is_known_scope("mcp:tools:call:write"));
        assert!(is_known_scope("mcp:resources:read"));
    }

    #[test]
    fn test_is_known_scope_invalid() {
        assert!(!is_known_scope(""));
        assert!(!is_known_scope("unknown:scope"));
        assert!(!is_known_scope("dns:delete"));
        assert!(!is_known_scope("pages:delete"));
        assert!(!is_known_scope("key:update"));
        assert!(!is_known_scope("auth:passkey:delete"));
    }

    #[test]
    fn test_known_scopes_completeness() {
        let list = known_scopes();
        // Every known scope should be recognized by is_known_scope
        for scope in &list {
            assert!(is_known_scope(scope), "Scope {} should be known", scope);
        }
        // is_known_scope should not have any scopes not in the list
        // (we check by round-tripping all known scopes)
        assert_eq!(list.len(), 19);
    }

    #[test]
    fn test_namespace_from_str() {
        assert_eq!(Namespace::from_str("auth"), Some(Namespace::Auth));
        assert_eq!(Namespace::from_str("key"), Some(Namespace::Key));
        assert_eq!(Namespace::from_str("jit"), Some(Namespace::Jit));
        assert_eq!(Namespace::from_str("mcp"), Some(Namespace::Mcp));
        assert_eq!(Namespace::from_str("dns"), Some(Namespace::Dns));
        assert_eq!(Namespace::from_str("pages"), Some(Namespace::Pages));
        assert_eq!(Namespace::from_str("unknown"), None);
    }

    #[test]
    fn test_namespace_as_str() {
        assert_eq!(Namespace::Auth.as_str(), "auth");
        assert_eq!(Namespace::Key.as_str(), "key");
        assert_eq!(Namespace::Jit.as_str(), "jit");
        assert_eq!(Namespace::Mcp.as_str(), "mcp");
        assert_eq!(Namespace::Dns.as_str(), "dns");
        assert_eq!(Namespace::Pages.as_str(), "pages");
    }

    #[test]
    fn test_namespace_of() {
        assert_eq!(namespace_of("dns:write"), Some(Namespace::Dns));
        assert_eq!(namespace_of("auth:passkey:register"), Some(Namespace::Auth));
        assert_eq!(namespace_of("mcp:tools:call:read"), Some(Namespace::Mcp));
        assert_eq!(namespace_of("admin"), None);
        assert_eq!(namespace_of("unknown:scope"), None);
    }
}
