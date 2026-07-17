//! Scope grammar and validation for capability tokens.
//!
//! ## Format
//!
//! Scopes use a colon-separated namespace hierarchy:
//!
//! ```text
//! "namespace:action"
//! ```
//!
//! Examples:
//! - `"dns:write"` — Write access to DNS records
//! - `"pages:deploy"` — Deploy pages
//! - `"mcp:tools:call:read"` — Read-only MCP tool calls
//!
//! ## Wildcard
//!
//! The special scope `"admin"` grants all permissions and always satisfies
//! any requirement check.

/// Check whether a set of token scopes satisfies a set of required scopes.
///
/// Returns `true` if **all** required scopes are covered by the token's
/// granted scopes. The wildcard scope `"admin"` always passes.
///
/// ## Examples
///
/// ```ignore
/// let granted = vec!["dns:read".to_string(), "pages:deploy".to_string()];
///
/// // All required are present:
/// assert!(satisfies(&granted, &["dns:read".to_string()]));
///
/// // Missing scope:
/// assert!(!satisfies(&granted, &["dns:write".to_string()]));
///
/// // Admin wildcard:
/// let admin = vec!["admin".to_string()];
/// assert!(satisfies(&admin, &["anything:at:all".to_string()]));
/// ```
#[allow(dead_code)]
pub fn satisfies(token_scopes: &[String], required: &[String]) -> bool {
    // Admin wildcard overrides all checks
    if token_scopes.iter().any(|s| s == "admin") {
        return true;
    }

    // Every required scope must be present in the token scopes
    required
        .iter()
        .all(|req| token_scopes.iter().any(|tok| tok == req))
}

/// Validate a scope string's format.
///
/// A valid scope is either:
/// - The literal string `"admin"` (wildcard)
/// - Two or more lowercase ASCII segments separated by colons,
///   e.g. `"dns:write"`, `"mcp:tools:call:read"`
///
/// Each segment must be non-empty and contain only lowercase ASCII letters.
///
/// ## Examples
///
/// ```ignore
/// assert!(is_valid_scope("admin"));
/// assert!(is_valid_scope("dns:write"));
/// assert!(is_valid_scope("mcp:tools:call:read"));
///
/// assert!(!is_valid_scope(""));
/// assert!(!is_valid_scope("no-colon"));
/// assert!(!is_valid_scope("Mixed:Case"));
/// assert!(!is_valid_scope(":leading-colon"));
/// assert!(!is_valid_scope("trailing-colon:"));
/// ```
#[allow(dead_code)]
pub fn is_valid_scope(s: &str) -> bool {
    // Wildcard
    if s == "admin" {
        return true;
    }

    // Must not be empty
    if s.is_empty() {
        return false;
    }

    // Split on colon — must have at least 2 segments
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() < 2 {
        return false;
    }

    // Every segment must be non-empty and lowercase ASCII
    parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_satisfies_exact_match() {
        let granted = vec!["dns:read".to_string(), "pages:deploy".to_string()];
        assert!(satisfies(&granted, &["dns:read".to_string()]));
        assert!(satisfies(&granted, &["pages:deploy".to_string()]));
        assert!(satisfies(
            &granted,
            &["dns:read".to_string(), "pages:deploy".to_string()]
        ));
    }

    #[test]
    fn test_satisfies_missing_scope() {
        let granted = vec!["dns:read".to_string()];
        assert!(!satisfies(&granted, &["dns:write".to_string()]));
    }

    #[test]
    fn test_satisfies_admin_wildcard() {
        let granted = vec!["admin".to_string()];
        assert!(satisfies(&granted, &["anything:at:all".to_string()]));
        assert!(satisfies(&granted, &[] as &[String]));
    }

    #[test]
    fn test_satisfies_empty_required() {
        let granted = vec!["dns:read".to_string()];
        assert!(satisfies(&granted, &[] as &[String]));
    }

    #[test]
    fn test_satisfies_empty_granted() {
        let granted: Vec<String> = vec![];
        assert!(!satisfies(&granted, &["dns:read".to_string()]));
    }

    #[test]
    fn test_is_valid_scope_valid() {
        assert!(is_valid_scope("admin"));
        assert!(is_valid_scope("dns:write"));
        assert!(is_valid_scope("pages:deploy"));
        assert!(is_valid_scope("mcp:tools:call:read"));
        assert!(is_valid_scope("dns:read"));
        assert!(is_valid_scope("auth:passkey:register"));
        assert!(is_valid_scope("key:create"));
        assert!(is_valid_scope("jit:mint"));
    }

    #[test]
    fn test_is_valid_scope_invalid() {
        assert!(!is_valid_scope(""));
        assert!(!is_valid_scope("no-colon"));
        assert!(!is_valid_scope("Mixed:Case"));
        assert!(!is_valid_scope("UPPER:lower"));
        assert!(!is_valid_scope(":leading-colon"));
        assert!(!is_valid_scope("trailing-colon:"));
        assert!(!is_valid_scope("double::colon"));
        assert!(!is_valid_scope("spaces: not allowed"));
        assert!(!is_valid_scope("digits:123"));
    }
}
