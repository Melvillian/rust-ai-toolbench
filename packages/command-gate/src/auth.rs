//! Passkey (`WebAuthn`) enrollment and login, plus server-side sessions.
//!
//! Single-owner model: one user handle, any number of passkeys (phone, laptop,
//! hardware key). New passkeys can only be enrolled with a one-time token that
//! is printed on the host's terminal, so reaching the web UI alone is never
//! enough to add a credential.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use webauthn_rs::prelude::{
    CreationChallengeResponse, Passkey, PasskeyAuthentication, PasskeyRegistration,
    PublicKeyCredential, RegisterPublicKeyCredential, RequestChallengeResponse, Uuid, Webauthn,
    WebauthnBuilder,
};

use crate::config::ServerConfig;

const CEREMONY_TTL: Duration = Duration::from_secs(120);
const MAX_PENDING: usize = 16;
const MAX_SESSIONS: usize = 32;
/// Hard cap regardless of activity: re-authenticate at least this often.
pub const SESSION_ABSOLUTE_MAX: Duration = Duration::from_secs(12 * 3600);
const STORE_FILE: &str = "passkeys.json";

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("webauthn configuration error: {0}")]
    Setup(String),
    #[error("credential store I/O error at {path}: {source}")]
    Store {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("credential store is corrupt: {0}")]
    Corrupt(serde_json::Error),
    #[error("{0}")]
    Insecure(String),
    /// Deliberately vague: callers map this to a generic 401.
    #[error("authentication failed")]
    Denied,
    #[error("no passkeys are enrolled yet; start the server with --enroll")]
    NoPasskeys,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoreFile {
    user_id: Uuid,
    passkeys: Vec<Passkey>,
}

#[derive(Debug)]
struct Session {
    created: Instant,
    last_seen: Instant,
    /// Tailscale identity that logged in; later requests must match.
    login: Option<String>,
}

#[derive(Debug)]
struct State {
    user_id: Uuid,
    passkeys: Vec<Passkey>,
    pending_reg: HashMap<String, (PasskeyRegistration, Instant)>,
    pending_auth: HashMap<String, (PasskeyAuthentication, Instant)>,
    /// Keyed by SHA-256 of the cookie value, so a memory dump or debug print
    /// never yields a usable token.
    sessions: HashMap<[u8; 32], Session>,
    enrollment: Option<(String, Instant)>,
}

#[derive(Debug)]
pub struct Auth {
    webauthn: Webauthn,
    store_path: PathBuf,
    session_idle: Duration,
    state: Mutex<State>,
}

impl Auth {
    /// Build the relying party and load (or initialise) the credential store
    /// under `cfg.data_dir`, which is created `0700` if missing.
    pub fn open(cfg: &ServerConfig) -> Result<Self, AuthError> {
        let webauthn = WebauthnBuilder::new(&cfg.rp_id, &cfg.origin)
            .and_then(|b| b.rp_name(&cfg.rp_name).build())
            .map_err(|e| AuthError::Setup(e.to_string()))?;

        ensure_private_dir(&cfg.data_dir)?;
        let store_path = cfg.data_dir.join(STORE_FILE);
        let store = match fs::read(&store_path) {
            Ok(bytes) => {
                check_private_file(&store_path)?;
                serde_json::from_slice::<StoreFile>(&bytes).map_err(AuthError::Corrupt)?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => StoreFile {
                user_id: Uuid::from_bytes(random_bytes()),
                passkeys: Vec::new(),
            },
            Err(source) => {
                return Err(AuthError::Store {
                    path: store_path,
                    source,
                });
            }
        };

        Ok(Self {
            webauthn,
            store_path,
            session_idle: Duration::from_secs(cfg.session_ttl_secs),
            state: Mutex::new(State {
                user_id: store.user_id,
                passkeys: store.passkeys,
                pending_reg: HashMap::new(),
                pending_auth: HashMap::new(),
                sessions: HashMap::new(),
                enrollment: None,
            }),
        })
    }

    pub fn passkey_count(&self) -> usize {
        self.lock().passkeys.len()
    }

    pub fn enrollment_open(&self) -> bool {
        self.lock()
            .enrollment
            .as_ref()
            .is_some_and(|(_, exp)| *exp > Instant::now())
    }

    /// Open a single-use enrollment window and return its secret token.
    pub fn open_enrollment(&self, ttl: Duration) -> String {
        let token = random_token();
        self.lock().enrollment = Some((token.clone(), Instant::now() + ttl));
        token
    }

    pub fn start_registration(
        &self,
        token: &str,
    ) -> Result<(String, CreationChallengeResponse), AuthError> {
        let mut st = self.lock();
        prune(&mut st);
        let valid = st.enrollment.as_ref().is_some_and(|(t, exp)| {
            *exp > Instant::now() && bool::from(t.as_bytes().ct_eq(token.as_bytes()))
        });
        if !valid {
            return Err(AuthError::Denied);
        }
        evict_oldest(&mut st.pending_reg);
        let exclude = st
            .passkeys
            .iter()
            .map(|p| p.cred_id().clone())
            .collect::<Vec<_>>();
        let (challenge, reg) = self
            .webauthn
            .start_passkey_registration(st.user_id, "owner", "remote-exec owner", Some(exclude))
            .map_err(|e| {
                tracing::warn!("start_passkey_registration: {e}");
                AuthError::Denied
            })?;
        let id = random_token();
        st.pending_reg.insert(id.clone(), (reg, Instant::now()));
        Ok((id, challenge))
    }

    pub fn finish_registration(
        &self,
        ceremony: &str,
        cred: &RegisterPublicKeyCredential,
    ) -> Result<(), AuthError> {
        let mut st = self.lock();
        prune(&mut st);
        let (reg, _) = st.pending_reg.remove(ceremony).ok_or(AuthError::Denied)?;
        // The enrollment window must still be open at finish time too.
        if st
            .enrollment
            .as_ref()
            .is_none_or(|(_, exp)| *exp <= Instant::now())
        {
            return Err(AuthError::Denied);
        }
        let passkey = self
            .webauthn
            .finish_passkey_registration(cred, &reg)
            .map_err(|e| {
                tracing::warn!("finish_passkey_registration: {e}");
                AuthError::Denied
            })?;
        st.passkeys.push(passkey);
        // Single use: one token, one passkey.
        st.enrollment = None;
        st.pending_reg.clear();
        self.persist(&st)?;
        tracing::info!("enrolled a new passkey ({} total)", st.passkeys.len());
        Ok(())
    }

    pub fn start_login(&self) -> Result<(String, RequestChallengeResponse), AuthError> {
        let mut st = self.lock();
        prune(&mut st);
        if st.passkeys.is_empty() {
            return Err(AuthError::NoPasskeys);
        }
        // Anyone who can reach the socket (incl. other local users, who can
        // forge headers on loopback) can start ceremonies; evicting the oldest
        // instead of refusing means they cannot lock the owner out.
        evict_oldest(&mut st.pending_auth);
        let (challenge, auth) = self
            .webauthn
            .start_passkey_authentication(&st.passkeys)
            .map_err(|e| {
                tracing::warn!("start_passkey_authentication: {e}");
                AuthError::Denied
            })?;
        let id = random_token();
        st.pending_auth.insert(id.clone(), (auth, Instant::now()));
        Ok((id, challenge))
    }

    /// Verify the assertion and mint a session token (the cookie value).
    pub fn finish_login(
        &self,
        ceremony: &str,
        cred: &PublicKeyCredential,
        login: Option<String>,
    ) -> Result<String, AuthError> {
        let mut st = self.lock();
        prune(&mut st);
        let (auth, _) = st.pending_auth.remove(ceremony).ok_or(AuthError::Denied)?;
        let result = self
            .webauthn
            .finish_passkey_authentication(cred, &auth)
            .map_err(|e| {
                tracing::warn!("finish_passkey_authentication: {e}");
                AuthError::Denied
            })?;
        // Persist updated signature counters / backup state. A counter that
        // fails to advance is rejected inside webauthn-rs as a possible clone.
        let mut changed = false;
        for pk in &mut st.passkeys {
            changed |= pk.update_credential(&result).unwrap_or(false);
        }
        if changed {
            self.persist(&st)?;
        }
        if st.sessions.len() >= MAX_SESSIONS {
            // Evict the stalest session rather than refusing the owner.
            if let Some(oldest) = st
                .sessions
                .iter()
                .min_by_key(|(_, s)| s.last_seen)
                .map(|(k, _)| *k)
            {
                st.sessions.remove(&oldest);
            }
        }
        let token = random_token();
        let now = Instant::now();
        st.sessions.insert(
            hash(&token),
            Session {
                created: now,
                last_seen: now,
                login,
            },
        );
        Ok(token)
    }

    /// Validate a session cookie, sliding its idle expiry forward.
    pub fn check_session(&self, token: &str, login: Option<&str>) -> bool {
        let mut st = self.lock();
        prune(&mut st);
        let idle = self.session_idle;
        let key = hash(token);
        let Some(s) = st.sessions.get_mut(&key) else {
            return false;
        };
        let now = Instant::now();
        if now.duration_since(s.last_seen) > idle
            || now.duration_since(s.created) > SESSION_ABSOLUTE_MAX
        {
            st.sessions.remove(&key);
            return false;
        }
        if s.login.as_deref() != login {
            return false;
        }
        s.last_seen = now;
        true
    }

    pub fn logout(&self, token: &str) {
        self.lock().sessions.remove(&hash(token));
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        // A panic while holding the lock leaves plain data behind; recover it
        // rather than taking the whole server down.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Atomic write: temp file (0600) then rename.
    fn persist(&self, st: &State) -> Result<(), AuthError> {
        let store = StoreFile {
            user_id: st.user_id,
            passkeys: st.passkeys.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&store).map_err(AuthError::Corrupt)?;
        let tmp = self.store_path.with_extension("json.tmp");
        // A stale temp file (e.g. from a crash) could carry a looser mode;
        // remove it and create fresh so 0600 always applies.
        let _ = fs::remove_file(&tmp);
        let io = |source| AuthError::Store {
            path: self.store_path.clone(),
            source,
        };
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(io)?;
        f.write_all(&bytes).map_err(io)?;
        f.sync_all().map_err(io)?;
        fs::rename(&tmp, &self.store_path).map_err(io)?;
        Ok(())
    }
}

fn evict_oldest<T>(map: &mut HashMap<String, (T, Instant)>) {
    while map.len() >= MAX_PENDING {
        let Some(oldest) = map
            .iter()
            .min_by_key(|(_, (_, t))| *t)
            .map(|(k, _)| k.clone())
        else {
            break;
        };
        map.remove(&oldest);
    }
}

fn prune(st: &mut State) {
    let now = Instant::now();
    st.pending_reg
        .retain(|_, (_, t)| now.duration_since(*t) < CEREMONY_TTL);
    st.pending_auth
        .retain(|_, (_, t)| now.duration_since(*t) < CEREMONY_TTL);
    if st.enrollment.as_ref().is_some_and(|(_, exp)| *exp <= now) {
        st.enrollment = None;
    }
}

fn ensure_private_dir(dir: &Path) -> Result<(), AuthError> {
    let io = |source| AuthError::Store {
        path: dir.to_path_buf(),
        source,
    };
    if !dir.exists() {
        fs::create_dir_all(dir).map_err(io)?;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(io)?;
    }
    let mode = fs::metadata(dir).map_err(io)?.permissions().mode();
    if mode & 0o022 != 0 {
        return Err(AuthError::Insecure(format!(
            "{} is group/world-writable (mode {mode:o}); chmod 700 it",
            dir.display()
        )));
    }
    Ok(())
}

fn check_private_file(path: &Path) -> Result<(), AuthError> {
    let mode = fs::metadata(path)
        .map_err(|source| AuthError::Store {
            path: path.to_path_buf(),
            source,
        })?
        .permissions()
        .mode();
    if mode & 0o077 != 0 {
        return Err(AuthError::Insecure(format!(
            "{} is readable by other users (mode {mode:o}); chmod 600 it",
            path.display()
        )));
    }
    Ok(())
}

fn hash(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf).expect("OS RNG unavailable");
    buf
}

/// 256 bits from the OS CSPRNG, hex-encoded.
pub fn random_token() -> String {
    use std::fmt::Write as _;
    random_bytes::<32>()
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Mode};

    fn cfg(dir: &Path) -> ServerConfig {
        let text = format!(
            "[server]\nrp_id=\"localhost\"\norigin=\"http://localhost:8787\"\ndata_dir={:?}\n\
             [[command]]\nid=\"x\"\nprogram=\"/bin/true\"\n",
            dir.join("data")
        );
        Config::from_toml(&text, Mode::Dev).unwrap().server
    }

    #[test]
    fn creates_private_data_dir_and_requires_token_for_enrollment() {
        let tmp = tempfile::tempdir().unwrap();
        let auth = Auth::open(&cfg(tmp.path())).unwrap();
        let mode = fs::metadata(tmp.path().join("data"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);

        assert!(matches!(
            auth.start_registration("nope"),
            Err(AuthError::Denied)
        ));
        let token = auth.open_enrollment(Duration::from_secs(60));
        assert!(auth.start_registration("wrong").is_err());
        assert!(auth.start_registration(&token).is_ok());
    }

    #[test]
    fn login_requires_enrolled_passkey_and_sessions_reject_unknown_tokens() {
        let tmp = tempfile::tempdir().unwrap();
        let auth = Auth::open(&cfg(tmp.path())).unwrap();
        assert!(matches!(auth.start_login(), Err(AuthError::NoPasskeys)));
        assert!(!auth.check_session("deadbeef", None));
    }

    #[test]
    fn rejects_world_readable_store() {
        let tmp = tempfile::tempdir().unwrap();
        let c = cfg(tmp.path());
        drop(Auth::open(&c).unwrap());
        let store = c.data_dir.join(STORE_FILE);
        fs::write(
            &store,
            r#"{"user_id":"00000000-0000-0000-0000-000000000000","passkeys":[]}"#,
        )
        .unwrap();
        fs::set_permissions(&store, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(Auth::open(&c), Err(AuthError::Insecure(_))));
    }

    #[test]
    fn tokens_are_unique_hex() {
        let a = random_token();
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_ne!(a, random_token());
    }
}
