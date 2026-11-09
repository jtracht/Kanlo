use super::*;
use argon2::{Algorithm, Argon2, Params, Version};
use subtle::ConstantTimeEq;

#[derive(Serialize, Deserialize)]
struct PasscodeHash {
    salt: String,
    hash: String,
}
#[derive(Serialize)]
pub struct Session {
    pub token: String,
    pub csrf: String,
    pub expires_at: String,
}
fn random_secret() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}
fn password_hash(passcode: &str, salt: &[u8]) -> Result<String> {
    let params = Params::new(19456, 2, 1, Some(32)).map_err(|_| Error::Storage)?;
    let mut hash = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passcode.as_bytes(), salt, &mut hash)
        .map_err(|_| Error::Storage)?;
    Ok(hex::encode(hash))
}
impl Store {
    pub fn has_passcode(&self) -> Result<bool> {
        self.connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM security_config WHERE key='passcode')",
                [],
                |r| r.get(0),
            )
            .map_err(storage)
    }
    pub fn set_passcode(&mut self, passcode: &str) -> Result<()> {
        if passcode.chars().count() < 12 || passcode.len() > 256 {
            return Err(Error::Validation(
                "Passcode requires 12–256 characters".into(),
            ));
        }
        let salt = Uuid::new_v4();
        let password = PasscodeHash {
            salt: hex::encode(salt.as_bytes()),
            hash: password_hash(passcode, salt.as_bytes())?,
        };
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        tx.execute("INSERT INTO security_config VALUES ('passcode',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[encode(&password)?]).map_err(storage)?;
        tx.execute("DELETE FROM external_sessions", [])
            .map_err(storage)?;
        audit(&tx, "passcode.changed", None)?;
        tx.commit().map_err(storage)?;
        Ok(())
    }
    pub fn login(&mut self, passcode: &str) -> Result<Session> {
        if passcode.len() > 256 {
            return Err(Error::Unauthorized);
        }
        let stored: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM security_config WHERE key='passcode'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        let Some(stored) = stored else {
            return Err(Error::Unauthorized);
        };
        let stored: PasscodeHash = decode(&stored)?;
        let salt = hex::decode(&stored.salt).map_err(|_| Error::Storage)?;
        let candidate = password_hash(passcode, &salt)?;
        if !bool::from(candidate.as_bytes().ct_eq(stored.hash.as_bytes())) {
            audit(&self.connection, "login.failed", None)?;
            return Err(Error::Unauthorized);
        }
        let token = random_secret();
        let csrf = hash(&format!("csrf:{token}"));
        let created = engine::now();
        let expires_at = (chrono::Utc::now() + chrono::Duration::hours(12))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        tx.execute(
            "DELETE FROM external_sessions WHERE expires_at<?1",
            [&created],
        )
        .map_err(storage)?;
        tx.execute(
            "INSERT INTO external_sessions VALUES (?1,?2,?3,?4)",
            params![hash(&token), hash(&csrf), expires_at, created],
        )
        .map_err(storage)?;
        audit(&tx, "login.success", None)?;
        tx.commit().map_err(storage)?;
        Ok(Session {
            token,
            csrf,
            expires_at,
        })
    }
    pub fn authorize_session(&self, token: &str, csrf: Option<&str>, write: bool) -> Result<()> {
        if token.len() != 64 {
            return Err(Error::Unauthorized);
        }
        let row: Option<(String, String)> = self
            .connection
            .query_row(
                "SELECT csrf_hash,expires_at FROM external_sessions WHERE token_hash=?1",
                [hash(token)],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(storage)?;
        let (expected, expires) = row.ok_or(Error::Unauthorized)?;
        if chrono::DateTime::parse_from_rfc3339(&expires).map_err(|_| Error::Storage)?
            <= chrono::Utc::now()
        {
            return Err(Error::Unauthorized);
        }
        if write {
            let csrf = csrf.filter(|s| s.len() == 64).ok_or(Error::Unauthorized)?;
            if !bool::from(hash(csrf).as_bytes().ct_eq(expected.as_bytes())) {
                return Err(Error::Unauthorized);
            }
        }
        Ok(())
    }
    pub fn logout(&mut self, token: &str) -> Result<()> {
        self.connection
            .execute(
                "DELETE FROM external_sessions WHERE token_hash=?1",
                [hash(token)],
            )
            .map_err(storage)?;
        audit(&self.connection, "session.revoked", None)
    }
    pub fn session_status(&self, token: &str) -> Result<Value> {
        self.authorize_session(token, None, false)?;
        Ok(json!({"authenticated":true,"csrf":hash(&format!("csrf:{token}"))}))
    }
    pub fn revoke_sessions(&mut self) -> Result<()> {
        self.connection
            .execute("DELETE FROM external_sessions", [])
            .map_err(storage)?;
        audit(&self.connection, "sessions.revoked", None)
    }
    pub fn mint_api_key(&mut self, name: &str) -> Result<Value> {
        let name = text(name, 120, "API key name")?;
        let id = Uuid::new_v4().to_string();
        let token = format!("{id}.{}", random_secret());
        self.connection
            .execute(
                "INSERT INTO api_keys VALUES (?1,?2,?3,?4,0)",
                params![id, name, hash(&token), engine::now()],
            )
            .map_err(storage)?;
        audit(&self.connection, "key.created", Some(&id))?;
        Ok(json!({"id":id,"name":name,"token":token,"shown_once":true}))
    }
    pub fn list_api_keys(&self) -> Result<Vec<Value>> {
        let mut query = self
            .connection
            .prepare("SELECT id,name,created_at,revoked FROM api_keys ORDER BY created_at")
            .map_err(storage)?;
        query.query_map([],|r|Ok(json!({"id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"created_at":r.get::<_,String>(2)?,"revoked":r.get::<_,bool>(3)?}))).map_err(storage)?.collect::<rusqlite::Result<_>>().map_err(storage)
    }
    pub fn revoke_api_key(&mut self, id: &str) -> Result<()> {
        let count = self
            .connection
            .execute("UPDATE api_keys SET revoked=1 WHERE id=?1", [id])
            .map_err(storage)?;
        if count == 0 {
            return Err(Error::NotFound);
        }
        audit(&self.connection, "key.revoked", Some(id))
    }
    pub fn authorize_key(&self, token: &str) -> Result<()> {
        if token.len() > 160 {
            return Err(Error::Unauthorized);
        }
        let id = token
            .split_once('.')
            .map(|(id, _)| id)
            .filter(|id| Uuid::parse_str(id).is_ok())
            .ok_or(Error::Unauthorized)?;
        let row: Option<(String, bool)> = self
            .connection
            .query_row(
                "SELECT secret_hash,revoked FROM api_keys WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(storage)?;
        let (expected, revoked) = row.ok_or(Error::Unauthorized)?;
        if revoked || !bool::from(hash(token).as_bytes().ct_eq(expected.as_bytes())) {
            return Err(Error::Unauthorized);
        }
        Ok(())
    }
}
fn audit(connection: &Connection, action: &str, subject: Option<&str>) -> Result<()> {
    connection
        .execute(
            "INSERT INTO security_audit(action,subject_id,created_at) VALUES (?1,?2,?3)",
            params![action, subject, engine::now()],
        )
        .map_err(storage)?;
    Ok(())
}
