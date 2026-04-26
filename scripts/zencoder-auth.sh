#!/usr/bin/env bash
#
# zencoder-auth.sh — Zencoder OAuth client_credentials token helper
#
# Why this exists:
#   IronClaw's `ironclaw tool auth <name>` supports two flows:
#     1. OAuth authorization_code + PKCE (browser redirect)
#     2. Manual "paste the token" prompt
#   Zencoder's token endpoint (https://fe.zencoder.ai/oauth/token) only
#   supports client_credentials — a server-to-server grant with no browser
#   redirect. So the exchange must happen outside IronClaw.
#
#   This script does the exchange, prints the resulting JWT, and tells you
#   exactly how to install it via `ironclaw tool auth zencoder-tool`.
#
# Usage:
#   scripts/zencoder-auth.sh [--client-id ID] [--client-secret SECRET]
#                            [--token-url URL]
#
# Flags:
#   --client-id ID         Use ID instead of prompting.
#   --client-secret SECRET Use SECRET instead of prompting (NOT recommended;
#                          appears in shell history / process list).
#   --token-url URL        Override the OAuth token endpoint
#                          (default: https://fe.zencoder.ai/oauth/token).
#   -h, --help             Show this help.
#
# Exit codes:
#   0   success (JWT printed and instructions shown)
#   1   bad usage / missing dependency
#   2   credential prompt aborted
#   3   token endpoint returned non-2xx
#   4   token response did not contain access_token

set -euo pipefail

TOKEN_URL_DEFAULT="https://fe.zencoder.ai/oauth/token"

print_help() {
    sed -n '2,35p' "$0" | sed 's/^# \{0,1\}//'
}

CLIENT_ID=""
CLIENT_SECRET=""
TOKEN_URL="$TOKEN_URL_DEFAULT"

while [ $# -gt 0 ]; do
    case "$1" in
        --client-id)        CLIENT_ID="$2"; shift 2 ;;
        --client-secret)    CLIENT_SECRET="$2"; shift 2 ;;
        --token-url)        TOKEN_URL="$2"; shift 2 ;;
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
            "$JSON_EXTRACTOR" -c 'import json,sys
d=json.load(sys.stdin)
v=d.get(sys.argv[1],"") if isinstance(d,dict) else ""
print("" if v is None else v)' "$1"
            ;;
        sed)
            input=$(cat)
            val=$(printf '%s' "$input" | sed -n "s/.*\"$1\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p" | head -n1)
            if [ -z "$val" ]; then
                val=$(printf '%s' "$input" | sed -n "s/.*\"$1\"[[:space:]]*:[[:space:]]*\\([0-9][0-9]*\\).*/\\1/p" | head -n1)
            fi
            printf '%s' "$val"
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
    if [ -t 0 ] && stty -echo 2>/dev/null; then
        restore_echo() { stty echo 2>/dev/null || true; printf '\n'; }
        trap 'restore_echo' INT TERM HUP EXIT
        printf "Zencoder Client Secret: "
        IFS= read -r CLIENT_SECRET || { trap - INT TERM HUP EXIT; restore_echo; exit 2; }
        trap - INT TERM HUP EXIT
        restore_echo
    else
        printf "Zencoder Client Secret (will be visible): "
        IFS= read -r CLIENT_SECRET || { echo; exit 2; }
    fi
fi
if [ -z "$CLIENT_SECRET" ]; then
    echo "ERROR: empty Client Secret." >&2
    exit 2
fi

# --- exchange ------------------------------------------------------------

build_json_body() {
    if command -v jq >/dev/null 2>&1; then
        jq -nc --arg id "$CLIENT_ID" --arg sec "$CLIENT_SECRET" \
            '{client_id:$id, client_secret:$sec, grant_type:"client_credentials"}'
    elif command -v python3 >/dev/null 2>&1; then
        python3 -c '
import json, sys
print(json.dumps({
    "client_id":     sys.argv[1],
    "client_secret": sys.argv[2],
    "grant_type":    "client_credentials",
}))
' "$CLIENT_ID" "$CLIENT_SECRET"
    else
        if printf '%s%s' "$CLIENT_ID" "$CLIENT_SECRET" | LC_ALL=C grep -q '[[:cntrl:]]'; then
            echo "ERROR: Client ID/Secret contains a control character (CR, LF, tab, …)." >&2
            echo "       Cannot construct safe JSON without jq or python3 — install one of:" >&2
            echo "         apt install -y jq        # or python3" >&2
            exit 1
        fi
        local id sec
        id=$(printf '%s' "$CLIENT_ID"     | sed 's/\\/\\\\/g; s/"/\\"/g')
        sec=$(printf '%s' "$CLIENT_SECRET" | sed 's/\\/\\\\/g; s/"/\\"/g')
        printf '{"client_id":"%s","client_secret":"%s","grant_type":"client_credentials"}' \
            "$id" "$sec"
    fi
}

JSON_BODY=$(build_json_body)

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

# --- print JWT + instructions -------------------------------------------

echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  ✓ JWT obtained successfully                                 ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
if [ -n "${EXPIRES_IN:-}" ]; then
    echo "  Token lifetime: ${EXPIRES_IN}s (re-run this script to rotate)"
    echo ""
fi
echo "  Your Zencoder access token:"
echo ""
printf '%s\n' "$ACCESS_TOKEN"
echo ""
echo "  ─────────────────────────────────────────────────────────────"
echo "  Next step — paste it into IronClaw:"
echo ""
echo "    ironclaw tool auth zencoder-tool"
echo ""
echo "  At the prompt:"
echo "    • Press 's' to skip the browser-open step (headless containers)"
echo "    • Paste the token above when asked 'Paste your token:'"
echo "    • IronClaw will validate it against the Zencoder API"
echo "  ─────────────────────────────────────────────────────────────"
