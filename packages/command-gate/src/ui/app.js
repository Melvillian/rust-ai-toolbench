// remote-exec UI. Plain DOM, no innerHTML: every server-provided string goes
// through textContent, so command output cannot inject markup. Client-side
// input patterns are UX only — the server validates everything again.
"use strict";

const $ = (id) => document.getElementById(id);

// ---- base64url <-> ArrayBuffer (WebAuthn JSON wire format) ----------------
function b64uToBuf(s) {
  s = s.replace(/-/g, "+").replace(/_/g, "/");
  while (s.length % 4) s += "=";
  const bin = atob(s);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out.buffer;
}
function bufToB64u(buf) {
  const bytes = new Uint8Array(buf);
  let bin = "";
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

// ---- API -------------------------------------------------------------------
async function api(path, body) {
  const init = body === undefined
    ? { method: "GET", credentials: "same-origin" }
    : {
        method: "POST",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      };
  const res = await fetch(path, init);
  let data = null;
  try { data = await res.json(); } catch { /* empty body */ }
  if (!res.ok) {
    const err = new Error((data && data.error) || `HTTP ${res.status}`);
    err.status = res.status;
    throw err;
  }
  return data;
}

function setStatus(msg, isError = false) {
  const el = $("status");
  el.textContent = msg || "";
  el.classList.toggle("error", isError);
}

async function busy(button, fn) {
  button.disabled = true;
  try {
    await fn();
  } catch (e) {
    setStatus(e.message || String(e), true);
    // Session expired mid-use: fall back to the sign-in screen.
    if (e.status === 401) refresh().catch(() => {});
  } finally {
    button.disabled = false;
  }
}

// ---- Passkeys --------------------------------------------------------------
function webauthnSupported() {
  return window.PublicKeyCredential && navigator.credentials;
}

async function enroll(token) {
  const { ceremony, options } = await api("/api/register/start", { token });
  const pk = options.publicKey;
  pk.challenge = b64uToBuf(pk.challenge);
  pk.user.id = b64uToBuf(pk.user.id);
  (pk.excludeCredentials || []).forEach((c) => { c.id = b64uToBuf(c.id); });

  const cred = await navigator.credentials.create({ publicKey: pk });
  const credential = {
    id: cred.id,
    rawId: bufToB64u(cred.rawId),
    type: cred.type,
    response: {
      attestationObject: bufToB64u(cred.response.attestationObject),
      clientDataJSON: bufToB64u(cred.response.clientDataJSON),
    },
    extensions: cred.getClientExtensionResults(),
  };
  await api("/api/register/finish", { ceremony, credential });
}

async function login() {
  const { ceremony, options } = await api("/api/login/start", {});
  const pk = options.publicKey;
  pk.challenge = b64uToBuf(pk.challenge);
  (pk.allowCredentials || []).forEach((c) => { c.id = b64uToBuf(c.id); });

  const cred = await navigator.credentials.get({ publicKey: pk });
  const r = cred.response;
  const credential = {
    id: cred.id,
    rawId: bufToB64u(cred.rawId),
    type: cred.type,
    response: {
      authenticatorData: bufToB64u(r.authenticatorData),
      clientDataJSON: bufToB64u(r.clientDataJSON),
      signature: bufToB64u(r.signature),
      userHandle: r.userHandle ? bufToB64u(r.userHandle) : null,
    },
    extensions: cred.getClientExtensionResults(),
  };
  await api("/api/login/finish", { ceremony, credential });
}

// ---- Commands --------------------------------------------------------------
function el(tag, props = {}, ...children) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (k === "text") node.textContent = v;
    else if (k === "class") node.className = v;
    else node.setAttribute(k, v);
  }
  for (const c of children) if (c) node.append(c);
  return node;
}

function paramInput(cmdId, p) {
  const id = `p-${cmdId}-${p.name}`;
  let input;
  let hint = p.description || "";
  switch (p.kind) {
    case "enum":
      input = el("select", { id, name: p.name });
      for (const v of p.values) input.append(el("option", { value: v, text: v }));
      break;
    case "integer":
      input = el("input", { id, name: p.name, type: "number", inputmode: "numeric", step: "1", min: String(p.min), max: String(p.max), required: "" });
      hint = [hint, `${p.min}–${p.max}`].filter(Boolean).join(" · ");
      break;
    case "word":
      input = el("input", { id, name: p.name, type: "text", autocapitalize: "off", autocomplete: "off", spellcheck: "false", required: "", maxlength: String(p.max_len), pattern: "[A-Za-z0-9_][A-Za-z0-9._\\-]*" });
      hint = [hint, "letters, digits, _ . -"].filter(Boolean).join(" · ");
      break;
    case "path":
      input = el("input", { id, name: p.name, type: "text", autocapitalize: "off", autocomplete: "off", spellcheck: "false", required: "", placeholder: "relative/path" });
      hint = [hint, "relative to the configured root"].filter(Boolean).join(" · ");
      break;
    default:
      input = el("input", { id, name: p.name, disabled: "" });
  }
  return el("div", { class: "field" },
    el("label", { for: id, text: p.name }),
    input,
    hint ? el("span", { class: "hint", text: hint }) : null);
}

function renderCommands(cmds) {
  const root = $("commands");
  root.replaceChildren();
  for (const c of cmds) {
    const fields = c.params.map((p) => paramInput(c.id, p));
    const run = el("button", { type: "button", text: "Run" });
    const card = el("details", { class: "card" },
      el("summary", {},
        el("span", { class: "id", text: c.id }),
        c.description ? el("span", { class: "desc", text: c.description }) : null),
      el("div", { class: "preview", text: c.preview }),
      ...fields,
      run);
    run.addEventListener("click", () => busy(run, async () => {
      const args = {};
      for (const f of card.querySelectorAll("input, select")) {
        if (f.checkValidity && !f.checkValidity()) { f.reportValidity(); return; }
        args[f.name] = f.value;
      }
      setStatus(`Running ${c.id}…`);
      const out = await api("/api/exec", { id: c.id, args });
      setStatus("");
      showResult(c.id, out);
    }));
    root.append(card);
  }
}

function showResult(id, out) {
  $("result").hidden = false;
  $("result-title").textContent = id;
  const meta = $("result-meta");
  let label;
  if (out.timed_out) label = "timed out";
  else if (out.exit_code !== null) label = `exit ${out.exit_code}`;
  else label = `signal ${out.signal}`;
  meta.textContent = `${label} · ${out.duration_ms} ms`;
  meta.className = "badge " + (out.exit_code === 0 ? "ok" : "fail");
  $("result-stdout").textContent = out.stdout + (out.stdout_truncated ? "\n[stdout truncated]" : "");
  $("result-stderr").textContent = out.stderr + (out.stderr_truncated ? "\n[stderr truncated]" : "");
  $("result").scrollIntoView({ behavior: "smooth", block: "start" });
}

// ---- Boot ------------------------------------------------------------------
async function refresh() {
  const s = await api("/api/session");
  $("who").textContent = s.login || "";
  const enrollToken = new URLSearchParams(location.hash.slice(1)).get("enroll");

  $("enroll").hidden = !(enrollToken && s.enrollment_open);
  $("login").hidden = s.authenticated || s.enrolled_passkeys === 0;
  $("logout").hidden = !s.authenticated;
  $("commands").hidden = !s.authenticated;
  if (!s.authenticated) $("result").hidden = true;

  if (enrollToken && !s.enrollment_open) {
    history.replaceState(null, "", location.pathname);
    setStatus("That enrollment link has expired or was already used.", true);
  } else if (s.enrolled_passkeys === 0 && !enrollToken) {
    setStatus("No passkey enrolled yet. Open the enrollment link printed on the server's terminal.");
  }
  if (s.authenticated) renderCommands(await api("/api/commands"));
}

document.addEventListener("DOMContentLoaded", () => {
  if (!webauthnSupported()) {
    setStatus("This browser does not support passkeys.", true);
    return;
  }
  $("enroll-btn").addEventListener("click", (e) => busy(e.currentTarget, async () => {
    const token = new URLSearchParams(location.hash.slice(1)).get("enroll");
    await enroll(token);
    // Drop the token from the address bar and history once it's spent.
    history.replaceState(null, "", location.pathname);
    setStatus("Passkey created. Sign in below.");
    await refresh();
  }));
  $("login-btn").addEventListener("click", (e) => busy(e.currentTarget, async () => {
    await login();
    setStatus("");
    await refresh();
  }));
  $("logout").addEventListener("click", (e) => busy(e.currentTarget, async () => {
    await api("/api/logout", {});
    await refresh();
  }));
  refresh().catch((e) => setStatus(e.message, true));
});
