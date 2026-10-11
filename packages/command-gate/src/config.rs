//! The command allowlist and server settings, loaded from a TOML file.
//!
//! Parsing is deliberately strict: unknown keys are rejected (a typo such as
//! `timout_secs` must fail loudly rather than silently fall back to a default),
//! and [`Config::from_toml`] validates every command before the server starts,
//! so a bad allowlist is a startup error, never a request-time surprise.

use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use webauthn_rs::prelude::Url;

/// Environment variables passed through to commands by default. Everything
/// else is cleared (`env_clear`), so secrets in the server's environment never
/// reach a child process unless listed here or in a command's `env` table.
const DEFAULT_PASS_ENV: &[&str] = &[
    "HOME",
    "USER",
    "LANG",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
];
const DEFAULT_BIND: &str = "127.0.0.1:8787";
const DEFAULT_SESSION_TTL_SECS: u64 = 15 * 60;
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_TIMEOUT_SECS: u64 = 60 * 60;
const DEFAULT_WORD_MAX_LEN: usize = 64;
const MAX_WORD_MAX_LEN: usize = 256;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not read config {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid config TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("invalid config: {0}")]
    Invalid(String),
}

fn invalid(msg: impl Into<String>) -> ConfigError {
    ConfigError::Invalid(msg.into())
}

// ---------------------------------------------------------------------------
// Raw (as written in TOML)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    server: RawServer,
    #[serde(default)]
    command: Vec<RawCommand>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawServer {
    bind: Option<SocketAddr>,
    rp_id: String,
    origin: String,
    rp_name: Option<String>,
    data_dir: PathBuf,
    #[serde(default)]
    allowed_tailscale_logins: Vec<String>,
    session_ttl_secs: Option<u64>,
    pass_env: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCommand {
    id: String,
    #[serde(default)]
    description: String,
    program: PathBuf,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    param: Vec<RawParam>,
    timeout_secs: Option<u64>,
    cwd: Option<PathBuf>,
    #[serde(default)]
    env: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawParam {
    name: String,
    kind: RawKind,
    #[serde(default)]
    description: String,
    values: Option<Vec<String>>,
    min: Option<i64>,
    max: Option<i64>,
    max_len: Option<usize>,
    root: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawKind {
    Enum,
    Integer,
    Word,
    Path,
}

// ---------------------------------------------------------------------------
// Validated
// ---------------------------------------------------------------------------

/// Whether the server trusts Tailscale identity headers (production) or runs
/// as a local-only dev server on `http://localhost`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Behind `tailscale serve`: HTTPS origin, `Tailscale-User-Login` required.
    Tailscale,
    /// Local development: `http://localhost` origin, no Tailscale check.
    Dev,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub commands: Vec<CommandSpec>,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub mode: Mode,
    pub bind: SocketAddr,
    pub rp_id: String,
    pub rp_name: String,
    pub origin: Url,
    /// `origin` serialised exactly as browsers send it in the `Origin` header
    /// (`scheme://host[:port]`, no trailing slash).
    pub origin_header: String,
    pub data_dir: PathBuf,
    pub allowed_tailscale_logins: BTreeSet<String>,
    pub session_ttl_secs: u64,
    pub pass_env: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandSpec {
    pub id: String,
    pub description: String,
    pub program: PathBuf,
    /// argv template: each element is a literal or exactly `{param}`.
    pub args: Vec<ArgTemplate>,
    pub params: Vec<ParamSpec>,
    pub timeout_secs: u64,
    #[serde(skip)]
    pub cwd: Option<PathBuf>,
    #[serde(skip)]
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "lowercase")]
pub enum ArgTemplate {
    Literal(String),
    Param(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct ParamSpec {
    pub name: String,
    pub description: String,
    #[serde(flatten)]
    pub kind: ParamKind,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ParamKind {
    /// One of a fixed set of operator-chosen strings.
    Enum { values: Vec<String> },
    /// A base-10 integer within `[min, max]`.
    Integer { min: i64, max: i64 },
    /// A conservative token: `[A-Za-z0-9_][A-Za-z0-9._-]*`, never starting
    /// with `-` or `.` (so it cannot become an option flag or `..`).
    Word { max_len: usize },
    /// A relative path resolved and canonicalised under `root`; the command
    /// receives the absolute canonical path. Each component must be a word.
    Path {
        #[serde(skip)]
        root: PathBuf,
    },
}

impl Config {
    /// Read and validate a config file.
    pub fn load(path: &Path, mode: Mode) -> Result<Self, ConfigError> {
        let read_err = |source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        };
        // The config *is* the security policy: whoever can edit it can
        // allowlist anything. Require it to be ours and not writable by others.
        let meta = std::fs::metadata(path).map_err(read_err)?;
        check_trusted_owner(&meta, &format!("config {}", path.display()))?;
        if meta.permissions().mode() & 0o022 != 0 {
            return Err(invalid(format!(
                "config {} is group/world-writable; chmod 600 it",
                path.display()
            )));
        }
        let text = std::fs::read_to_string(path).map_err(read_err)?;
        Self::from_toml(&text, mode)
    }

    /// Parse and validate config TOML. Filesystem checks (program exists and is
    /// executable, path roots exist) run here, so this is not a pure function.
    pub fn from_toml(text: &str, mode: Mode) -> Result<Self, ConfigError> {
        let raw: RawConfig = toml::from_str(text)?;
        let server = validate_server(raw.server, mode)?;
        let mut seen = BTreeSet::new();
        let mut commands = Vec::with_capacity(raw.command.len());
        for cmd in raw.command {
            if !seen.insert(cmd.id.clone()) {
                return Err(invalid(format!("duplicate command id {:?}", cmd.id)));
            }
            commands.push(validate_command(cmd)?);
        }
        if commands.is_empty() {
            return Err(invalid("no [[command]] entries defined"));
        }
        Ok(Self { server, commands })
    }

    pub fn command(&self, id: &str) -> Option<&CommandSpec> {
        self.commands.iter().find(|c| c.id == id)
    }
}

fn validate_server(raw: RawServer, mode: Mode) -> Result<ServerConfig, ConfigError> {
    let bind = match raw.bind {
        Some(b) => b,
        None => DEFAULT_BIND.parse().expect("default bind address is valid"),
    };
    // The Tailscale identity header is only trustworthy if nothing but
    // `tailscaled` (a local process) can reach us. Binding to the tailnet IP
    // or 0.0.0.0 would let any peer forge `Tailscale-User-Login`.
    if !bind.ip().is_loopback() {
        return Err(invalid(format!(
            "server.bind must be a loopback address (got {bind}); expose it with `tailscale serve`, \
             which is what makes the identity header trustworthy"
        )));
    }

    let origin = Url::parse(&raw.origin).map_err(|e| invalid(format!("server.origin: {e}")))?;
    let host = origin
        .host_str()
        .ok_or_else(|| invalid("server.origin has no host"))?
        .to_owned();
    if origin.path() != "/" || origin.query().is_some() || origin.fragment().is_some() {
        return Err(invalid(
            "server.origin must be scheme://host[:port] with no path",
        ));
    }
    if host != raw.rp_id && !host.ends_with(&format!(".{}", raw.rp_id)) {
        return Err(invalid(format!(
            "server.rp_id {:?} must equal or be a parent domain of the origin host {host:?}",
            raw.rp_id
        )));
    }
    match mode {
        Mode::Tailscale => {
            if origin.scheme() != "https" {
                return Err(invalid(
                    "server.origin must be https:// (use `tailscale serve` for a cert)",
                ));
            }
            if raw.allowed_tailscale_logins.is_empty() {
                return Err(invalid(
                    "server.allowed_tailscale_logins must list at least one login (e.g. \"you@github\")",
                ));
            }
        }
        Mode::Dev => {
            if host != "localhost" {
                return Err(invalid(
                    "--dev requires server.origin to be http://localhost:<port>",
                ));
            }
        }
    }
    if !raw.data_dir.is_absolute() {
        return Err(invalid("server.data_dir must be an absolute path"));
    }
    let session_ttl_secs = raw.session_ttl_secs.unwrap_or(DEFAULT_SESSION_TTL_SECS);
    if !(60..=24 * 3600).contains(&session_ttl_secs) {
        return Err(invalid(
            "server.session_ttl_secs must be between 60 and 86400",
        ));
    }
    let pass_env = raw
        .pass_env
        .unwrap_or_else(|| DEFAULT_PASS_ENV.iter().map(|s| (*s).to_owned()).collect());
    for key in &pass_env {
        check_env_key(key)?;
    }
    let mut origin_header = origin.as_str().to_owned();
    if origin_header.ends_with('/') {
        origin_header.pop();
    }

    Ok(ServerConfig {
        mode,
        bind,
        rp_id: raw.rp_id,
        rp_name: raw.rp_name.unwrap_or_else(|| "remote-exec".to_owned()),
        origin,
        origin_header,
        data_dir: raw.data_dir,
        allowed_tailscale_logins: raw.allowed_tailscale_logins.into_iter().collect(),
        session_ttl_secs,
        pass_env,
    })
}

fn validate_command(raw: RawCommand) -> Result<CommandSpec, ConfigError> {
    let id = raw.id;
    let ctx = |msg: String| invalid(format!("command {id:?}: {msg}"));

    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(ctx("id must match [a-z0-9-]{1,64}".into()));
    }
    check_program(&raw.program).map_err(ctx)?;
    let timeout_secs = raw.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS);
    if !(1..=MAX_TIMEOUT_SECS).contains(&timeout_secs) {
        return Err(ctx(format!("timeout_secs must be 1..={MAX_TIMEOUT_SECS}")));
    }
    if let Some(cwd) = &raw.cwd
        && (!cwd.is_absolute() || !cwd.is_dir())
    {
        return Err(ctx("cwd must be an existing absolute directory".into()));
    }
    for key in raw.env.keys() {
        check_env_key(key).map_err(|e| ctx(e.to_string()))?;
    }

    let mut params = Vec::with_capacity(raw.param.len());
    let mut names = BTreeSet::new();
    for p in raw.param {
        if !is_param_name(&p.name) {
            return Err(ctx(format!(
                "param name {:?} must match [a-z][a-z0-9_]{{0,31}}",
                p.name
            )));
        }
        if !names.insert(p.name.clone()) {
            return Err(ctx(format!("duplicate param {:?}", p.name)));
        }
        params.push(validate_param(p).map_err(|e| ctx(e.to_string()))?);
    }

    let mut used = BTreeSet::new();
    let mut args = Vec::with_capacity(raw.args.len());
    for a in raw.args {
        if a.contains('\0') {
            return Err(ctx("args may not contain NUL".into()));
        }
        if let Some(name) = a.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
            if !names.contains(name) {
                return Err(ctx(format!("arg {a:?} references undeclared param")));
            }
            used.insert(name.to_owned());
            args.push(ArgTemplate::Param(name.to_owned()));
        } else if a.contains('{') || a.contains('}') {
            // Keep substitution whole-element only: "--file={x}" style splicing
            // makes it too easy to reason wrongly about what the child sees.
            return Err(ctx(format!(
                "arg {a:?}: placeholders must be an entire argv element, e.g. \"{{name}}\""
            )));
        } else {
            args.push(ArgTemplate::Literal(a));
        }
    }
    if let Some(unused) = names.difference(&used).next() {
        return Err(ctx(format!(
            "param {unused:?} is declared but never used in args"
        )));
    }

    Ok(CommandSpec {
        id,
        description: raw.description,
        program: raw.program,
        args,
        params,
        timeout_secs,
        cwd: raw.cwd,
        env: raw.env,
    })
}

fn validate_param(p: RawParam) -> Result<ParamSpec, ConfigError> {
    let RawParam {
        name,
        kind,
        description,
        values,
        min,
        max,
        max_len,
        root,
    } = p;
    let unexpected = |field: &str, present: bool| {
        if present {
            Err(invalid(format!(
                "param {name:?}: `{field}` is not valid for this kind"
            )))
        } else {
            Ok(())
        }
    };
    let kind = match kind {
        RawKind::Enum => {
            unexpected("min/max", min.is_some() || max.is_some())?;
            unexpected("max_len", max_len.is_some())?;
            unexpected("root", root.is_some())?;
            let values = values.unwrap_or_default();
            if values.is_empty() {
                return Err(invalid(format!(
                    "param {name:?}: enum needs non-empty `values`"
                )));
            }
            if values
                .iter()
                .any(|v| v.is_empty() || v.chars().any(char::is_control))
            {
                return Err(invalid(format!(
                    "param {name:?}: enum values must be non-empty, no control chars"
                )));
            }
            ParamKind::Enum { values }
        }
        RawKind::Integer => {
            unexpected("values", values.is_some())?;
            unexpected("max_len", max_len.is_some())?;
            unexpected("root", root.is_some())?;
            let (Some(min), Some(max)) = (min, max) else {
                return Err(invalid(format!(
                    "param {name:?}: integer needs `min` and `max`"
                )));
            };
            if min > max {
                return Err(invalid(format!("param {name:?}: min > max")));
            }
            // A negative value would start with '-', i.e. look like an option.
            if min < 0 {
                return Err(invalid(format!("param {name:?}: integer min must be >= 0")));
            }
            ParamKind::Integer { min, max }
        }
        RawKind::Word => {
            unexpected("values", values.is_some())?;
            unexpected("min/max", min.is_some() || max.is_some())?;
            unexpected("root", root.is_some())?;
            let max_len = max_len.unwrap_or(DEFAULT_WORD_MAX_LEN);
            if !(1..=MAX_WORD_MAX_LEN).contains(&max_len) {
                return Err(invalid(format!(
                    "param {name:?}: max_len must be 1..={MAX_WORD_MAX_LEN}"
                )));
            }
            ParamKind::Word { max_len }
        }
        RawKind::Path => {
            unexpected("values", values.is_some())?;
            unexpected("min/max", min.is_some() || max.is_some())?;
            unexpected("max_len", max_len.is_some())?;
            let root = root.ok_or_else(|| invalid(format!("param {name:?}: path needs `root`")))?;
            if !root.is_absolute() {
                return Err(invalid(format!("param {name:?}: root must be absolute")));
            }
            // Canonicalise once at startup so the containment check at request
            // time compares like with like (no symlinked root surprises).
            let root = root
                .canonicalize()
                .map_err(|e| invalid(format!("param {name:?}: root {}: {e}", root.display())))?;
            if !root.is_dir() {
                return Err(invalid(format!("param {name:?}: root is not a directory")));
            }
            ParamKind::Path { root }
        }
    };
    Ok(ParamSpec {
        name,
        description,
        kind,
    })
}

/// Absolute, executable, and not swappable by other users.
fn check_program(program: &Path) -> Result<(), String> {
    // An absolute program path means no PATH lookup at exec time: the binary
    // that runs is exactly the one you allowlisted.
    if !program.is_absolute() {
        return Err("program must be an absolute path".into());
    }
    let shown = program.display();
    let meta = std::fs::metadata(program).map_err(|e| format!("program {shown}: {e}"))?;
    if !meta.is_file() || meta.permissions().mode() & 0o111 == 0 {
        return Err(format!("program {shown} is not an executable file"));
    }
    // Resolve symlinks, then require the binary and every directory above it
    // to be untamperable by other users: otherwise they could swap the
    // program after it was allowlisted.
    let real = program
        .canonicalize()
        .map_err(|e| format!("program {shown}: {e}"))?;
    let real_meta = std::fs::metadata(&real).map_err(|e| e.to_string())?;
    check_trusted_owner(&real_meta, &format!("program {}", real.display()))
        .map_err(|e| e.to_string())?;
    if real_meta.permissions().mode() & 0o022 != 0 {
        return Err(format!(
            "program {} is group/world-writable",
            real.display()
        ));
    }
    for dir in real.ancestors().skip(1) {
        let m = std::fs::metadata(dir).map_err(|e| e.to_string())?;
        if m.permissions().mode() & 0o002 != 0 {
            return Err(format!(
                "program directory {} is world-writable",
                dir.display()
            ));
        }
    }
    Ok(())
}

/// Owned by root or by the user this server runs as.
fn check_trusted_owner(meta: &std::fs::Metadata, what: &str) -> Result<(), ConfigError> {
    let uid = meta.uid();
    if uid == 0 || uid == nix::unistd::geteuid().as_raw() {
        Ok(())
    } else {
        Err(invalid(format!(
            "{what} is owned by uid {uid}, not root or the current user"
        )))
    }
}

fn is_param_name(s: &str) -> bool {
    let mut bytes = s.bytes();
    s.len() <= 32
        && bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn check_env_key(key: &str) -> Result<(), ConfigError> {
    let mut bytes = key.bytes();
    let ok = bytes
        .next()
        .is_some_and(|b| b.is_ascii_uppercase() || b == b'_')
        && bytes.all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_');
    if ok {
        Ok(())
    } else {
        Err(invalid(format!(
            "env var name {key:?} must match [A-Z_][A-Z0-9_]*"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server_block() -> &'static str {
        r#"
[server]
rp_id = "box.tail1234.ts.net"
origin = "https://box.tail1234.ts.net"
data_dir = "/tmp/command-gate-test"
allowed_tailscale_logins = ["alex@github"]
"#
    }

    fn with_command(cmd: &str) -> String {
        format!("{}\n{cmd}", server_block())
    }

    #[test]
    fn parses_a_valid_command() {
        let cfg = Config::from_toml(
            &with_command(
                r#"
[[command]]
id = "echo-word"
program = "/bin/echo"
args = ["hello", "{who}"]
[[command.param]]
name = "who"
kind = "word"
"#,
            ),
            Mode::Tailscale,
        )
        .unwrap();
        let cmd = cfg.command("echo-word").unwrap();
        assert_eq!(cmd.args[1], ArgTemplate::Param("who".into()));
        assert_eq!(cfg.server.origin_header, "https://box.tail1234.ts.net");
    }

    #[test]
    fn rejects_non_loopback_bind() {
        let text = server_block().replace("[server]", "[server]\nbind = \"0.0.0.0:8787\"")
            + "[[command]]\nid=\"x\"\nprogram=\"/bin/true\"\n";
        let err = Config::from_toml(&text, Mode::Tailscale).unwrap_err();
        assert!(err.to_string().contains("loopback"), "{err}");
    }

    #[test]
    fn rejects_relative_program_and_spliced_placeholders() {
        let rel = with_command("[[command]]\nid=\"x\"\nprogram=\"echo\"\n");
        assert!(Config::from_toml(&rel, Mode::Tailscale).is_err());

        let spliced = with_command(
            "[[command]]\nid=\"x\"\nprogram=\"/bin/echo\"\nargs=[\"--n={n}\"]\n[[command.param]]\nname=\"n\"\nkind=\"word\"\n",
        );
        let err = Config::from_toml(&spliced, Mode::Tailscale).unwrap_err();
        assert!(err.to_string().contains("entire argv element"), "{err}");
    }

    #[test]
    fn rejects_unknown_keys_and_unused_params() {
        let typo = with_command("[[command]]\nid=\"x\"\nprogram=\"/bin/true\"\ntimout_secs=5\n");
        assert!(Config::from_toml(&typo, Mode::Tailscale).is_err());

        let unused = with_command(
            "[[command]]\nid=\"x\"\nprogram=\"/bin/true\"\n[[command.param]]\nname=\"n\"\nkind=\"word\"\n",
        );
        let err = Config::from_toml(&unused, Mode::Tailscale).unwrap_err();
        assert!(err.to_string().contains("never used"), "{err}");
    }

    #[test]
    fn rejects_negative_integer_min() {
        let neg = with_command(
            "[[command]]\nid=\"x\"\nprogram=\"/bin/echo\"\nargs=[\"{n}\"]\n[[command.param]]\nname=\"n\"\nkind=\"integer\"\nmin=-5\nmax=5\n",
        );
        let err = Config::from_toml(&neg, Mode::Tailscale).unwrap_err();
        assert!(err.to_string().contains(">= 0"), "{err}");
    }

    #[test]
    fn rejects_program_in_world_writable_dir() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("tool");
        std::fs::copy("/bin/true", &bin).unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
        let text = with_command(&format!(
            "[[command]]\nid=\"x\"\nprogram=\"{}\"\n",
            bin.display()
        ));
        let err = Config::from_toml(&text, Mode::Tailscale).unwrap_err();
        assert!(err.to_string().contains("world-writable"), "{err}");
    }

    #[test]
    fn tailscale_mode_requires_https_and_allowed_logins() {
        let http = with_command("[[command]]\nid=\"x\"\nprogram=\"/bin/true\"\n")
            .replace("https://", "http://");
        assert!(Config::from_toml(&http, Mode::Tailscale).is_err());

        let no_logins = with_command("[[command]]\nid=\"x\"\nprogram=\"/bin/true\"\n")
            .replace("allowed_tailscale_logins = [\"alex@github\"]", "");
        assert!(Config::from_toml(&no_logins, Mode::Tailscale).is_err());
    }
}
