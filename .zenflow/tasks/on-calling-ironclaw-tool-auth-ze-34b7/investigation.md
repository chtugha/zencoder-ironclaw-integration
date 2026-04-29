# Investigation — `ironclaw tool auth zencoder-tool` failure

## Bug summary

Two related symptoms reported by the user:

1. **`ironclaw tool auth zencoder-tool` is interactive and broken on a headless container.**
   It prints the setup banner, fails to open a browser (`os error 2` — no browser binary available), then prompts `Paste your token:` and aborts with `✗ Interrupted`.

2. **The "manual fallback" `curl` command in the README returns `{"errors":["Invalid input json"],"errorCode":"ER-00006"}`.**
   Reproduced against `https://fe.zencoder.ai/oauth/token`: with valid JSON the endpoint returns `{"errors":["Invalid authentication"]}` for bad credentials. The user's failing command contained typographic curly quotes (`\\“ … \\“`) instead of straight quotes (`\\"`), which made the body invalid JSON. So symptom #2 is a copy/paste artifact, not a code bug — but the README example is brittle and easy to misuse.

The deeper problem is symptom #1: the README's "Step 8" promises an automatic OAuth2 `client_credentials` token exchange via `ironclaw tool auth`, but **IronClaw does not implement that grant type**.

## Root cause analysis

### IronClaw's `tool auth` flow

Source: `nearai/ironclaw` `src/cli/tool.rs` and `src/tools/wasm/capabilities_schema.rs` (staging branch).

`auth_tool()` (cli/tool.rs:719) decides between two flows based on the parsed `auth` capability:

```rust
if let Some(ref oauth) = auth.oauth {
    return auth_tool_oauth(...).await;   // browser-based authorization_code + PKCE
}
auth_tool_manual(...).await              // setup_url + "Paste your token:"
```

`auth_tool_oauth` is hard-wired to the **OAuth 2.0 authorization_code flow with PKCE**: it spins up a localhost callback listener, opens `oauth.authorization_url` in a browser, waits for `code` on the redirect, and exchanges it at `oauth.token_url`. There is no code path in IronClaw 0.25/0.26 that performs a `client_credentials` exchange.

The `AuthCapabilitySchema` in `capabilities_schema.rs:614` recognises only these top-level fields: `secret_name`, `display_name`, `oauth`, `instructions`, `setup_url`, `token_hint`, `env_var`, `provider`, `validation_endpoint`. Inside `oauth` (`OAuthConfigSchema`, line 659) the required fields are `authorization_url` and `token_url` — there is no `grant_type`.

### Our `zencoder-tool.capabilities.json` `auth` section

```json
"auth": {
  "type": "oauth2",
  "grant_type": "client_credentials",
  "token_url": "https://fe.zencoder.ai/oauth/token",
  "client_id_secret": "zencoder_client_id",
  "client_secret_secret": "zencoder_client_secret",
  "secret_name": "zencoder_access_token",
  "display_name": "Zencoder",
  "instructions": "...",
  "setup_url": "https://auth.zencoder.ai"
}
```

Of these, only `secret_name`, `display_name`, `instructions`, and `setup_url` are recognised by IronClaw. `type`, `grant_type`, `token_url`, `client_id_secret`, and `client_secret_secret` are silently ignored by serde because they are not declared on `AuthCapabilitySchema`.

Crucially, **there is no nested `oauth` object**, so `auth.oauth` is `None` and IronClaw falls through to `auth_tool_manual`. That function:

1. Prints the instructions.
2. Tries to open `setup_url` in a browser (`open::that(...)`) — fails on headless Debian containers with `No such file or directory (os error 2)` because no `xdg-open`/`open` binary is installed.
3. Prompts `Paste your token:` and reads from stdin.
4. The user has nothing meaningful to paste (auth.zencoder.ai issues `client_id` + `client_secret`, not a JWT), and pressing Ctrl-C / EOF results in `✗ Interrupted`.

### Why Zencoder cannot use IronClaw's `oauth` flow either

Zencoder's `https://fe.zencoder.ai/oauth/token` only supports `grant_type=client_credentials` (Personal Access Token style). It has **no authorization endpoint** and no consent UI for the redirect-based flow. So we cannot simply add an `oauth` block — there is no `authorization_url` to point at.

The WASM tool itself also cannot perform the exchange:

* The tool only has `secret_exists()`, not `secret_get()`, on the host bridge (lib.rs:298 confirms this); raw secret values are never injected into the sandbox by design.
* `client_credentials` requires the secret to travel in the request **body**, which IronClaw's credential injector cannot construct (it only injects headers / bearer tokens).
* `fe.zencoder.ai` is not in the HTTP allowlist anyway.

### Misleading documentation

`README.md` Step 8 currently states:

> ```bash
> ironclaw tool auth zencoder-tool
> ```
> This exchanges your Client ID and Client Secret for a JWT access token via Zencoder's OAuth2 `client_credentials` flow. IronClaw handles token injection automatically — credentials never enter the WASM sandbox.

This is incorrect — IronClaw does not perform that exchange. The "Manual token fallback" block lower down does work but is presented as a niche backup.

The README's curl example also uses `\"…\"` escapes inside double quotes; users on terminals with smart-quote autocorrect (the original bug report shows `\\“`) end up sending invalid JSON and seeing `ER-00006`.

### Phantom `zencoder_api_key` entry in `info` output

`ironclaw tool info zencoder-tool` shows `Secrets (existence check only): zencoder_api_key`. This comes from `capabilities.secrets.allowed_names`, where `zencoder_api_key` is still listed for backward-compatibility with the legacy non-OAuth path (lib.rs:307 still references it). It is harmless but adds noise; the legacy path is unreachable now that OAuth is the only documented option.

## Affected components

* `zencoder-tool/zencoder-tool.capabilities.json` — bogus `auth.type` / `auth.grant_type` / `auth.token_url` / `auth.client_id_secret` / `auth.client_secret_secret`; `setup.required_secrets` for `zencoder_client_id` and `zencoder_client_secret` that are never used by either IronClaw or the tool.
* `README.md` — Step 7 (`ironclaw tool setup`) collects unused secrets; Step 8 advertises a non-existent `client_credentials` flow inside `ironclaw tool auth`; the manual curl block is the only path that actually works and should be promoted to primary.
* `zencoder-tool/src/lib.rs` — error messages in `ensure_auth_configured()` (lib.rs:316–335) and the 401 handler (lib.rs:412–414) instruct the user to run `ironclaw tool auth zencoder-tool`, which will not work.

## Proposed solution

1. **Make the auth section a clean manual-token flow that IronClaw actually understands.**
   Edit `zencoder-tool.capabilities.json`:
   * Drop `type`, `grant_type`, `token_url`, `client_id_secret`, `client_secret_secret` from `auth`.
   * Keep `secret_name: zencoder_access_token`, `display_name`, `setup_url: https://auth.zencoder.ai`.
   * Rewrite `instructions` to walk the user through (a) creating a PAT at auth.zencoder.ai, (b) running the curl exchange, (c) pasting the resulting `access_token` JWT.
   * Add `token_hint: "JWT — three base64url segments separated by dots"` for display.

     *Field-handling verification (2026-04-26):* `AuthCapabilitySchema.token_hint` is declared `Option<String>` in `capabilities_schema.rs:640`. Existing real-world examples in the same file are free-text (`"Starts with 'sk-'"`, `"Starts with 'secret_' or 'ntn_'"` — lines 608 and 1058). It is **not** a regex/format validator — only a human-readable hint shown next to the prompt. The proposed value is correct as-is; no escaping or pattern conversion needed.
   * Add `env_var: "ZENCODER_ACCESS_TOKEN"` so headless installs can pre-populate via env.
   * Add a `validation_endpoint` calling `GET https://api.zencoder.ai/api/v1/projects` with `success_status: 200` so `ironclaw tool auth` validates the pasted token.

   *Endpoint verification (2026-04-26):* probed `/api/v1/projects`, `/api/v1/users/me`, `/api/v1/me`, `/api/v1/health` — all return `401` without a token (proves they exist; gateway gates everything on auth). `/api/v1/projects` is the recommended choice because it is already exercised by `list_projects` in this tool, so we know it returns `200` for any token with normal scopes. If a less-privileged ping is preferred, `/api/v1/users/me` also responded; either is acceptable. The implementation step should send a real test request with a freshly-issued token before committing the choice.

2. **Drop the unused client-id/secret secrets.**
   Remove `zencoder_client_id` and `zencoder_client_secret` from both `setup.required_secrets` and `capabilities.secrets.allowed_names`. They were only ever needed because we falsely promised an in-IronClaw `client_credentials` exchange; with the manual curl flow they live only on the user's shell.

3. **Remove `zencoder_api_key` from `secrets.allowed_names`** (and the corresponding legacy branch in `ensure_auth_configured`) — it is dead code post-OAuth migration.

   *Verified call sites (2026-04-26):* `rg zencoder_api_key zencoder-tool/src/lib.rs` returns exactly one hit at `lib.rs:303` — the `let has_legacy_key = near::agent::host::secret_exists("zencoder_api_key");` branch and its warning log (lines 303–311). Removing the secret from `allowed_names` is safe **iff** lines 303–311 are deleted in the same commit. The warning text itself is also stale ("client_id + client_secret") and would need rewriting if the branch were kept — another reason to drop it outright.

4. **Rewrite README Step 7 + Step 8.**
   * Collapse setup + auth into one step: "Generate token → exchange via curl → `ironclaw secret set zencoder_access_token <jwt>`".
   * Rewrite the curl example so it is robust against smart-quote autocorrect: prefer single-quoted JSON (`-d '{"client_id": "…", "client_secret": "…", "grant_type": "client_credentials"}'`) and put it in a clearly labelled fenced block. Add a one-liner that uses `jq -r .access_token` to feed `ironclaw secret set` directly so users never copy the token by hand.
   * Single-quoted shell strings are bash/zsh/fish only. Add a **PowerShell equivalent** for Windows users, e.g.
     ```powershell
     $body = @{ client_id = '...'; client_secret = '...'; grant_type = 'client_credentials' } | ConvertTo-Json
     $resp = Invoke-RestMethod -Uri 'https://fe.zencoder.ai/oauth/token' -Method Post -ContentType 'application/json' -Body $body
     ironclaw secret set zencoder_access_token $resp.access_token
     ```
     and a `cmd.exe` note steering users to PowerShell or WSL.
   * Optionally mention `ironclaw tool auth zencoder-tool` as a "paste-the-JWT" shortcut once the capabilities are fixed.
   * Add an explicit note: "IronClaw does not currently support OAuth `client_credentials`; the curl exchange is required."

5. **Update error messages in `lib.rs`** to point at the new procedure (curl + `ironclaw secret set zencoder_access_token`) instead of `ironclaw tool auth zencoder-tool`.

### Edge cases / side effects

* Existing users who already ran `ironclaw tool setup` and stored `zencoder_client_id` / `zencoder_client_secret`: removing those names from `secrets.allowed_names` will leave the values orphaned in IronClaw's secret store but won't break anything — they can be cleaned up with `ironclaw secret unset`. Mention this in the README upgrade note.
* Validation endpoint is `/api/v1/projects` (decision finalised in §1 above; pending only a smoke test with a real token at implementation time). It requires the token to have project-list scope; if it doesn't, the validator will reject it — acceptable, since surfacing bad tokens at auth time is the whole point.
* The `env_var` fallback (`ZENCODER_ACCESS_TOKEN`) makes CI / headless setups trivial — no terminal prompt needed.
* No WASM-side code changes are required for the auth fix itself; only the user-facing error strings and the capabilities manifest. Tests in `lib.rs` should not regress.

## Reproduction

```bash
# Symptom 1 — interactive prompt fails on headless host
ironclaw tool auth zencoder-tool
# → Could not open browser: No such file or directory (os error 2)
# → Paste your token:
# → ✗ Interrupted

# Symptom 2 — smart-quote curl
curl -s -X POST https://fe.zencoder.ai/oauth/token \
  -H "Content-Type: application/json" \
  -d "{\"client_id\": \"…\", \"client_secret\": \"…\“, \"grant_type\": \"client_credentials\"}"
# → {"errors":["Invalid input json"],"errorCode":"ER-00006"}

# Same payload with straight quotes works (returns Invalid authentication
# for bad creds, which proves the endpoint accepts JSON):
curl -s -X POST https://fe.zencoder.ai/oauth/token \
  -H 'Content-Type: application/json' \
  -d '{"client_id":"x","client_secret":"x","grant_type":"client_credentials"}'
# → {"errors":["Invalid authentication"]}
```

## Implementation notes (2026-04-26)

Applied the proposed fix exactly as outlined in §1–§5:

* **`zencoder-tool/zencoder-tool.capabilities.json`** — replaced the bogus
  `auth` block with a clean manual-token spec: `secret_name`,
  `display_name`, `setup_url`, `env_var: ZENCODER_ACCESS_TOKEN`,
  `token_hint`, `validation_endpoint` (`GET /api/v1/projects`,
  `success_status: 200`), and a multi-step `instructions` string that
  walks the user through the curl exchange. Pruned
  `secrets.allowed_names` to just `zencoder_access_token`. Dropped the
  entire `setup` block (no required secrets remain). JSON re-validated
  with `python3 -m json.tool`.
* **`zencoder-tool/src/lib.rs`** —
  * `description()` now describes the manual JWT-paste flow instead of
    promising an in-IronClaw `client_credentials` exchange.
  * `ensure_auth_configured()` simplified to a single
    `secret_exists("zencoder_access_token")` check; the legacy
    `zencoder_api_key` warning branch and the
    `zencoder_client_id`/`zencoder_client_secret` checks are gone (they
    were dead code once the OAuth-in-IronClaw fiction was removed).
    Error message now includes the curl one-liner.
  * 401 handler updated to point users at the curl exchange + reinstall
    flow rather than `ironclaw tool auth zencoder-tool` alone.
* **`README.md`** — rewrote Step 7 ("Obtain a Zencoder access token") to
  explain that IronClaw cannot perform `client_credentials` and to
  document a robust bash/zsh one-liner (single-quoted JSON, jq pipe
  straight into `ironclaw secret set`) plus a PowerShell equivalent and
  a `cmd.exe` note. Step 8 now describes how the simplified
  `ironclaw tool auth zencoder-tool` flow works (paste-the-JWT,
  validation against `/api/v1/projects`, `s` to skip the browser on
  headless hosts) and how to use the `ZENCODER_ACCESS_TOKEN` env-var
  fallback for CI. Added an upgrade note showing how to clean up the
  now-orphaned `zencoder_client_id` / `zencoder_client_secret` /
  `zencoder_api_key` secrets.

## Review follow-up (2026-04-26, second pass)

A cross-review flagged two issues with the first implementation pass.
Both are now fixed:

1. **Blank-line bug in the curl block of the
   `ensure_auth_configured()` error message.** The original used
   `\\\n\` + ` \n       -H ...` patterns; the `\\\n` emitted a
   backslash + newline, the trailing `\` line-continuation ate the
   source newline + leading whitespace, and the next ` \n` injected
   a *second* newline before `-H`. Result: every shell-continuation
   `\` was followed by a blank line, which breaks `bash`/`zsh`
   line-continuation semantics — copy-pasted commands would have
   failed silently.
   * Replaced the brittle escape-soup with a multi-line Rust raw
     string constant `AUTH_NOT_CONFIGURED_ERROR` (regular `"…"`
     literal with literal newlines; the only escapes left are `\\`
     for the shell continuation char and `\"` for JSON quotes inside
     the curl `-d` arg).
   * Step ordering inside the message now matches README priority:
     `ironclaw secret set zencoder_access_token <jwt>` first,
     `ironclaw tool auth zencoder-tool` second (interactive
     paste-the-JWT alternative).
2. **Missing regression tests.** Added three targeted unit tests in
   `lib.rs::tests`:
   * `test_auth_not_configured_error_has_no_blank_continuation_lines`
     — walks the rendered error string line-by-line; for every line
     ending with `\`, asserts the next line is non-empty. This test
     **fails against the pre-fix string** (proven by reverting the
     constant locally) and passes against the new constant.
   * `test_auth_not_configured_error_mentions_required_commands` —
     guards the user-facing affordances: `fe.zencoder.ai/oauth/token`,
     `ironclaw secret set zencoder_access_token`,
     `ironclaw tool auth zencoder-tool`, `client_credentials`. Catches
     accidental future deletions of any of those references.
   * `test_auth_not_configured_error_uses_single_quoted_json` —
     asserts the curl `-d` argument starts with `'{"client_id":` so a
     refactor cannot reintroduce double-quoted JSON (which would be
     vulnerable to the smart-quote autocorrect failure mode that
     produced the original `ER-00006` bug report).

Also addressed the suggestions from review:

* README Step 7 one-liner now has an inline comment explaining the
  `'"$VAR"'` quoting trick and reiterating the smart-quote-safety
  reason for single-quoted JSON.
* Step ordering in the lib.rs error message aligned with README
  (secret-set first, tool-auth second).

(The third suggestion — annotating `validation_endpoint` semantics —
is left as a maintenance note here rather than committed to source: as
of IronClaw 0.25/0.26 the validation request is performed by the host
CLI, not the WASM sandbox, so the host's normal HTTPS stack handles it
and the sandbox allowlist does not gate it. The absolute URL in
`capabilities.json` is correct for the host; if IronClaw ever moves
this call into the sandbox, the existing allowlist entry
`api.zencoder.ai` + `/api/v1/` already covers it. No action needed.)

## Root cause: `ironclaw secret set` does not exist

After user testing confirmed the helper scripts still failed, the IronClaw CLI
documentation was consulted at https://mintlify.wiki/logicminds/ironclaw/cli/config
and https://mintlify.wiki/logicminds/ironclaw/cli/tool.

**Finding:** IronClaw has NO `ironclaw secret` subcommand at all. The full CLI
is: `run`, `onboard`, `config`, `tool`, `registry`, `mcp`, `memory`, `pairing`,
`service`, `status`, `doctor`, `completion`. Secrets are set exclusively via:
- `ironclaw tool auth <name>` — interactive prompt to paste the auth token
- `ironclaw tool setup <name>` — interactive prompts for additional secrets

All calls to `ironclaw secret set zencoder_access_token` in both helper scripts
failed because the command does not exist.

## Final fix (2026-04-26, fourth pass)

Removed `ironclaw secret set` from both scripts entirely. The scripts now:
1. Prompt for Client ID + Client Secret
2. Exchange them for a JWT via curl to `fe.zencoder.ai/oauth/token`
3. Print the JWT prominently
4. Instruct the user to run `ironclaw tool auth zencoder-tool` and paste it

Files changed:
- `scripts/zencoder-auth.sh` — removed `--print-only`, `--secret-name`,
  `--no-set` flags and all `ironclaw secret set` invocations; output now
  prints the token and clear paste instructions
- `scripts/zencoder-auth.ps1` — same; removed `$SecretName`, `$PrintOnly`
  params and the ironclaw invocation block
- `README.md` — Section 7 rewritten as a 3-step flow (generate PAT → run
  helper → paste via `ironclaw tool auth`); manual fallback updated to print
  the JWT and refer to Step 3; all `ironclaw secret set` references removed
- `zencoder-tool/src/lib.rs` — `AUTH_NOT_CONFIGURED_ERROR` and 401 handler
  updated to match new flow; two tests updated to assert against new content

## Final audit pass (2026-04-29)

Comprehensive codebase audit covering bugs, stubs, magic numbers, dead code,
security issues, and WASM sandbox HTTP restriction compliance.

### Fixes applied

1. **`api_request` — `Content-Type: application/json` on bodyless requests**
   (`lib.rs` lines 352-361): GET/HEAD/OPTIONS requests were sent with a
   `Content-Type: application/json` header despite having no body. Strict
   proxies or WAFs may reject this. Fixed: header map is now conditional —
   `Content-Type` is only included when `body.is_some()`.

2. **`url_encode_path` — undersized capacity hint** (`lib.rs` line 145):
   `String::with_capacity(s.len() * 2)` was incorrect; worst case is 3x
   (every byte → `%XX`). Changed to `s.len() * 3`.

3. **`PlanStepSummary` — dead `Deserialize` derive** (`lib.rs` line 275):
   This struct is only ever serialized (in `check_solution_status`), never
   deserialized. Removed the unused `Deserialize` derive.

4. **README.md — stale "API key" wording** (line 403): Security section
   referred to "API key" but the tool uses a JWT access token. Fixed to
   "access token".

5. **README.md — wrong test count** (line 418): Stated "73 tests" but there
   are 78. Updated to "78 tests" and line count to "2000+".

### Items audited and confirmed correct

- **WASM HTTP sandbox compliance**: all `api_request` calls use
  `api.zencoder.ai` with methods `GET`, `POST`, `PATCH` only — matches
  `capabilities.json` allowlist. No calls to `fe.zencoder.ai` or any
  other host from WASM code. `fe.zencoder.ai` is only used by the helper
  scripts which run outside the sandbox.
- **`secret_exists` only** — no `secret_get` calls. Correct per WIT.
- **Idempotent retry guard**: `is_idempotent` correctly limits retries to
  `GET | HEAD | OPTIONS`. POST/PATCH run exactly once.
- **Named constants**: all magic numbers extracted (`MAX_HTTP_ATTEMPTS`,
  `HTTP_TIMEOUT_MS`, `RATE_LIMIT_WARN_THRESHOLD`, `ERROR_BODY_PREVIEW_CHARS`,
  `UUID_LEN`, `UUID_SEGMENT_LENS`, `SCHEDULE_TIME_LEN`, `HOUR_MAX`,
  `MINUTE_MAX`, `DAY_OF_WEEK_MAX`, `TITLE_MAX_CHARS`, `TITLE_MIN_WORD_BREAK`,
  `MAX_TEXT_LENGTH`, `DEFAULT_WORKFLOW_ID`).
- **URL encoding**: `url_encode_path` correctly preserves RFC 3986 unreserved
  chars (A-Z, a-z, 0-9, `-`, `_`, `.`, `~`) and percent-encodes everything
  else byte-by-byte, including multi-byte UTF-8.
- **UUID validation**: strict 8-4-4-4-12 hex check prevents path traversal.
- **Input length bounds**: all text fields checked against 64KB cap.
- **`derive_title`**: multi-byte-safe char-boundary truncation with word
  break heuristic. Tests cover edge cases.
- **`capabilities.json`**: clean manual-token auth spec with validation
  endpoint. No stale/bogus fields.
- **Helper scripts**: jq→python3→sed fallback chain correct; stty trap
  handles SIGINT/SIGTERM/HUP; control-char rejection in sed fallback;
  no `ironclaw secret set` references remain.
- **Skill v1.3.0**: keywords/patterns within IronClaw caps (20/5);
  composition rules for 12+ native skills; resilience state machine
  with lazy probe and exponential backoff.

## Test results

* `cargo test` (workspace `zencoder-tool`): **78 passed; 0 failed; 0 ignored**
* `cargo fmt --check`: clean
* `cargo clippy --all --all-features`: clean
