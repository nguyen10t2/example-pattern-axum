use dsa::utils::jwt::JwtConfig;
use uuid::Uuid;

#[test]
fn test_jwt_access_and_refresh_token_lifecycle() {
    let jwt = JwtConfig {
        secret: "test_secret_key_12345678901234567890".to_string(),
        issuer: "splitdebt".to_string(),
        audience: "splitdebt-users".to_string(),
        access_token_expiration_secs: 900,
        refresh_token_expiration_secs: 604800,
    };

    let user_id = Uuid::now_v7();
    let access_token = jwt.gen_access_token(user_id).unwrap();
    let claims = jwt.verify_access_token(&access_token).unwrap();
    assert_eq!(claims.sub, user_id.to_string());

    let jti = Uuid::now_v7().to_string();
    let refresh_token = jwt.gen_refresh_token(user_id, &jti).unwrap();
    let refresh_claims = jwt.verify_refresh_token(&refresh_token).unwrap();
    assert_eq!(refresh_claims.sub, user_id.to_string());
    assert_eq!(refresh_claims.jti, jti);
}
