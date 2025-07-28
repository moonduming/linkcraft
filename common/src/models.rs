use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: u64,  // user id
    pub exp: i64, // 过期时间(Unix 秒)
    pub jti: String, // JWT ID
}