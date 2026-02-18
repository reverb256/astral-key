//! Astral Key - CORS middleware
//!
//! Cross-Origin Resource Sharing configuration.

use tower_http::cors::{Any, CorsLayer};

/// Create CORS middleware
pub fn create_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any) // In production, specify allowed origins
        .allow_methods(Any)
        .allow_headers(Any)
        .max_age(std::time::Duration::from_secs(3600))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_cors_layer() {
        let cors = create_cors_layer();
        // Test that the layer is created
        // Actual testing would require sending HTTP requests
    }
}
