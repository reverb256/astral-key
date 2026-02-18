//! Unit tests for JWT authentication

#[cfg(test)]
mod tests {
    use super::super::*;
    use std::time::Duration;

    fn create_test_service() -> JwtService {
        let secret = b"test_secret_key_32_bytes_long_!!!";
        JwtService::new(secret, Duration::from_secs(900), Duration::from_secs(604800))
            .unwrap()
    }

    #[test]
    fn test_generate_access_token() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();

        let token = service.generate_access_token(user_id);
        assert!(token.is_ok());

        let token_str = token.unwrap();
        assert!(!token_str.is_empty());
        assert_eq!(token_str.split('.').count(), 3); // JWT has 3 parts
    }

    #[test]
    fn test_generate_refresh_token() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();

        let token = service.generate_refresh_token(user_id);
        assert!(token.is_ok());

        let token_str = token.unwrap();
        assert!(!token_str.is_empty());
        assert_eq!(token_str.split('.').count(), 3);
    }

    #[test]
    fn test_generate_token_pair() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();

        let tokens = service.generate_token_pair(user_id);
        assert!(tokens.is_ok());

        let pair = tokens.unwrap();
        assert!(!pair.access_token.is_empty());
        assert!(!pair.refresh_token.is_empty());
        assert_ne!(pair.access_token, pair.refresh_token);
    }

    #[test]
    fn test_validate_access_token() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();

        let token = service.generate_access_token(user_id).unwrap();
        let claims = service.validate_access_token(&token);

        assert!(claims.is_ok());
        assert_eq!(claims.unwrap().sub, user_id.to_string());
    }

    #[test]
    fn test_validate_refresh_token() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();

        let token = service.generate_refresh_token(user_id).unwrap();
        let claims = service.validate_refresh_token(&token);

        assert!(claims.is_ok());
        assert_eq!(claims.unwrap().sub, user_id.to_string());
    }

    #[test]
    fn test_refresh_token_cannot_be_used_as_access() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();

        let refresh_token = service.generate_refresh_token(user_id).unwrap();
        let result = service.validate_access_token(&refresh_token);

        assert!(result.is_err());
    }

    #[test]
    fn test_access_token_cannot_be_used_as_refresh() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();

        let access_token = service.generate_access_token(user_id).unwrap();
        let result = service.validate_refresh_token(&access_token);

        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_token_rejected() {
        let service = create_test_service();

        let result = service.validate_access_token("invalid.token.here");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_user_id() {
        let service = create_test_service();
        let user_id = Uuid::new_v4();

        let token = service.generate_access_token(user_id).unwrap();
        let extracted_id = service.extract_user_id(&token);

        assert!(extracted_id.is_ok());
        assert_eq!(extracted_id.unwrap(), user_id);
    }

    #[test]
    fn test_jwt_secret_too_short() {
        let short_secret = b"short";
        let result = JwtService::new(short_secret, Duration::from_secs(900), Duration::from_secs(604800));

        assert!(result.is_err());
    }
}
