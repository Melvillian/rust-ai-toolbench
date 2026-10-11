# remote-exec

Run a fixed allowlist of commands on your machine from your phone. Reachable only over your tailnet, only by allowlisted Tailscale identities, only after a passkey sign-in — and even then, only commands you listed, with arguments that pass strict typed validation, executed without a shell.

```
phone (Safari/Chrome, passkey)
   │  HTTPS, valid cert, tailnet only
   ▼
tailscaled  ── `tailscale serve` adds Tailscale-User-Login ──▶ 127.0.0.1:8787  remote-exec
                                                                │ passkey session check
                                                                │ allowlist + typed args
                                                                ▼
                                                     execve(program, argv)  (no shell)
```

## Setup

1. **Tailscale.** Enable MagicDNS and HTTPS certificates for your tailnet (admin console → DNS). Note your machine name, e.g. `mybox.tail1234.ts.net`, and your login (`tailscale whois $(tailscale ip -4 <phone>)` or the admin console Users page).

2. **Install.**
   ```bash
   cargo install --path apps/remote-exec
   mkdir -p ~/.config/remote-exec
   cp apps/remote-exec/commands.example.toml ~/.config/remote-exec/commands.toml
   chmod 600 ~/.config/remote-exec/commands.toml
   $EDITOR ~/.config/remote-exec/commands.toml   # rp_id, origin, logins, data_dir, commands
   remote-exec --config ~/.config/remote-exec/commands.toml --check
   ```

3. **Expose via Tailscale** (HTTPS on 443 → loopback):
   ```bash
   tailscale serve --bg 8787
   ```
   Do **not** use `tailscale funnel` — that is public internet, and funnel traffic carries no user identity, so every request would be refused anyway.

4. **Enroll your phone's passkey.** Run in a terminal (not the service), so you can see the one-time link:
   ```bash
   remote-exec --config ~/.config/remote-exec/commands.toml --enroll
   ```
   Open the printed `https://…#enroll=<token>` link on your phone (AirDrop/scan it — don't paste it into a chat app), tap **Create passkey**, then **Sign in**. The token is single-use and expires in 15 minutes. Repeat with `--enroll` for each extra device; enroll at least two so losing your phone doesn't lock you out.

5. **Run as a service.** Stop the interactive run, then follow the header of `remote-exec.service`. Add `ReadWritePaths=` for every directory your commands write to.

Local development without Tailscale: set `rp_id = "localhost"`, `origin = "http://localhost:8787"` and pass `--dev`.

## Writing commands safely

- Prefer `enum` params. Use `word`/`path` only when the value set is genuinely open.
- Never allowlist anything that executes its arguments: shells, `python -c`, `env`, `sudo`, `xargs`, `find -exec`, `git -c`, `ssh`, `awk`. Argument validation cannot make an interpreter safe.
- Add `"--"` before trailing free-form args where the program supports it.
- If you need logic or pipes, put it in a reviewed script owned by you, mode `755`, and allowlist the script.
- `path` params resolve symlinks and must stay inside `root`. Anything writable under that root by someone else is still theirs to point at.

## Operational notes

- Audit log: `<data_dir>/audit.jsonl` — every authenticated attempt, including rejected arguments.
- Passkeys are bound to `rp_id`. Renaming the machine or tailnet means re-enrolling.
- The process refuses to run as root; give it a user whose permissions are the blast radius you accept.
- One command runs at a time; a second request gets HTTP 409 until it finishes.
