#!/usr/bin/env bash
#
# zencoder-auth.sh — one-shot Zencoder OAuth client_credentials helper
#
# Why this exists:
#   IronClaw's `ironclaw tool auth <name>` does NOT implement the OAuth2
#   client_credentials grant (its built-in flows are authorization_code+PKCE
#   and "paste the token"). Zencoder's token endpoint only supports
#   client_credentials. So the exchange has to happen outside IronClaw.
#
#   This script does the exchange, then installs the resulting JWT as the
#   IronClaw secret `zencoder_access_token` so the WASM tool can use it.
#
# Usage:
#   scripts/zencoder-auth.sh [--client-id ID] [--client-secret SECRET]
#                            [--secret-name NAME] [--print-only]
#                            [--token-url URL] [--no-set]
#
# Flags:
#   --client-id ID         Use ID instead of prompting.
#   --client-secret SECRET Use SECRET instead of prompting (NOT recommended;
#                          appears in shell history / process list).
#   --secret-name NAME     IronClaw secret to write (default: zencoder_access_token).
#   --token-url URL        Override the OAuth token endpoint
#                          (default: https://fe.zencoder.ai/oauth/token).
#   --print-only           Print the JWT and DO NOT call `ironclaw secret set`.
#                          Useful on machines without IronClaw, or for piping.
#   --no-set               Alias for --print-only.
#   -h, --help             Show this help.
#
# Exit codes:
#   0   success
#   1   bad usage / missing dependency
#   2   credential prompt aborted
#   3   token endpoint returned non-2xx
#   4   token response did not contain access_token
#   5   `ironclaw secret set` failed

set -euo pipefail

TOKEN_URL_DEFAULT="https://fe.zencoder.ai/oauth/token"
SECRET_NAME_DEFAULT="zencoder_access_token"

print_help() {
    sed -n '2,40p' "$0" | sed 's/^# \{0,1\}//'
}

CLIENT_ID=""
CLIENT_SECRET=""
SECRET_NAME="$SECRET_NAME_DEFAULT"
TOKEN_URL="$TOKEN_URL_DEFAULT"
PRINT_ONLY=0

while [ $# -gt 0 ]; do
    case "$1" in
        --client-id)        CLIENT_ID="$2"; shift 2 ;;
        --client-secret)    CLIENT_SECRET="$2"; shift 2 ;;
        --secret-name)      SECRET_NAME="$2"; shift 2 ;;
        --token-url)        TOKEN_URL="$2"; shift 2 ;;
        --print-only|--no-set) PRINT_ONLY=1; shift ;;
        -h|--help)          print_help; exit 0 ;;
        *) echo "Unknown argument: $1" >&2; print_help >&2; exit 1 ;;
    esac
done

# --- dependency checks ---------------------------------------------------

need() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "ERROR: required command not found: $1" >&2
        exit 1
    }
}
need curl

if [ "$PRINT_ONLY" -eq 0 ]; then
    if ! command -v ironclaw >/dev/null 2>&1; then
        echo "ERROR: 'ironclaw' not in PATH. Install it first, or rerun with --print-only" >&2
        echo "       to print the JWT and install it manually." >&2
        exit 1
    fi
fi

# Pick a JSON parser. Prefer jq, fall back to python3, fall back to sed.
JSON_EXTRACTOR=""
if command -v jq >/dev/null 2>&1; then
    JSON_EXTRACTOR="jq"
elif command -v python3 >/dev/null 2>&1; then
    JSON_EXTRACTOR="python3"
elif command -v python >/dev/null 2>&1; then
    JSON_EXTRACTOR="python"
else
    JSON_EXTRACTOR="sed"
    echo "Warning: neither jq nor python found; falling back to sed parser." >&2
    echo "         The sed parser is fragile — install jq for safety." >&2
fi

extract_json_field() {
    # $1 = field name, reads JSON from stdin
    case "$JSON_EXTRACTOR" in
        jq)
            jq -r --arg f "$1" '.[$f] // empty'
            ;;
        python3|python)
            "$JSON_EXTRACTOR" -c "import json,sys; d=json.load(sys.stdin); print(d.get('$1','') if isinstance(d,dict) else '')"
            ;;
        sed)
            sed -n "s/.*\"$1\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p" | head -n1
            ;;
    esac
}

# --- prompts -------------------------------------------------------------

if [ -z "$CLIENT_ID" ]; then
    if [ ! -t 0 ]; then
        echo "ERROR: --client-id not supplied and stdin is not a terminal." >&2
        exit 2
    fi
    printf "Zencoder Client ID: "
    IFS= read -r CLIENT_ID || { echo; exit 2; }
fi
if [ -z "$CLIENT_ID" ]; then
    echo "ERROR: empty Client ID." >&2
    exit 2
fi

if [ -z "$CLIENT_SECRET" ]; then
    if [ ! -t 0 ]; then
        echo "ERROR: --client-secret not supplied and stdin is not a terminal." >&2
        exit 2
    fi
    # Read with echo disabled.
    if [ -t 0 ] && stty -echo 2>/dev/null; then
        printf "Zencoder Client Secret: "
        IFS= read -r CLIENT_SECRET || { stty echo; echo; exit 2; }
        stty echo
        echo
    else
        # Fallback for shells without stty.
        printf "Zencoder Client Secret (will be visible): "
        IFS= read -r CLIENT_SECRET || { echo; exit 2; }
    fi
fi
if [ -z "$CLIENT_SECRET" ]; then
    echo "ERROR: empty Client Secret." >&2
    exit 2
fi

# --- exchange ------------------------------------------------------------

# Build the JSON body with python or printf — never with shell interpolation
# directly into single-quoted curl args, so users can't be tripped up by
# special characters in the secret.
build_json_body() {
    if command -v python3 >/dev/null 2>&1; then
        python3 -c '
import json, sys
print(json.dumps({
    "client_id":     sys.argv[1],
    "client_secret": sys.argv[2],
    "grant_type":    "client_credentials",
}))
' "$CLIENT_ID" "$CLIENT_SECRET"
    else
        # Best-effort escape: backslash-escape backslashes and double quotes.
        local id sec
        id=$(printf '%s' "$CLIENT_ID"     | sed 's/\\/\\\\/g; s/"/\\"/g')
        sec=$(printf '%s' "$CLIENT_SECRET" | sed 's/\\/\\\\/g; s/"/\\"/g')
        printf '{"client_id":"%s","client_secret":"%s","grant_type":"client_credentials"}' \
            "$id" "$sec"
    fi
}

JSON_BODY=$(build_json_body)

# Send the request and capture body + http status separately.
# `--data-binary` avoids any --data quirks; -w prints the trailing status.
HTTP_RESPONSE=$(
    printf '%s' "$JSON_BODY" |
        curl -sS -X POST "$TOKEN_URL" \
            -H 'Content-Type: application/json' \
            -H 'Accept: application/json' \
            --data-binary @- \
            -w '\n__HTTP_STATUS__:%{http_code}'
) || {
    echo "ERROR: curl failed to reach $TOKEN_URL" >&2
    exit 3
}

HTTP_BODY=${HTTP_RESPONSE%$'\n'__HTTP_STATUS__:*}
HTTP_STATUS=${HTTP_RESPONSE##*__HTTP_STATUS__:}

if [ "$HTTP_STATUS" -lt 200 ] || [ "$HTTP_STATUS" -ge 300 ]; then
    echo "ERROR: token endpoint returned HTTP $HTTP_STATUS" >&2
    echo "Response body:" >&2
    echo "$HTTP_BODY" >&2
    exit 3
fi

ACCESS_TOKEN=$(printf '%s' "$HTTP_BODY" | extract_json_field access_token)
EXPIRES_IN=$(printf '%s' "$HTTP_BODY"   | extract_json_field expires_in || true)

if [ -z "$ACCESS_TOKEN" ]; then
    echo "ERROR: response did not contain an access_token field." >&2
    echo "Response body:" >&2
    echo "$HTTP_BODY" >&2
    exit 4
fi

# --- install or print ----------------------------------------------------

if [ "$PRINT_ONLY" -eq 1 ]; then
    printf '%s\n' "$ACCESS_TOKEN"
    exit 0
fi

if ! printf '%s' "$ACCESS_TOKEN" | ironclaw secret set "$SECRET_NAME" --stdin >/dev/null 2>&1; then
    # Fallback: not every IronClaw build supports --stdin. Try positional.
    if ! ironclaw secret set "$SECRET_NAME" "$ACCESS_TOKEN" >/dev/null 2>&1; then
        echo "ERROR: 'ironclaw secret set $SECRET_NAME ...' failed." >&2
        echo "       Token (copy this and set it manually):" >&2
        printf '%s\n' "$ACCESS_TOKEN" >&2
        exit 5
    fi
fi

echo "OK: stored Zencoder JWT in IronClaw secret '$SECRET_NAME'."
if [ -n "${EXPIRES_IN:-}" ]; then
    echo "    Token lifetime: ${EXPIRES_IN}s (re-run this script to rotate)."
fi
echo "    Validate with: ironclaw tool auth zencoder-tool"
