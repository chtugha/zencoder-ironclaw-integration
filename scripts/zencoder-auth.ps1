<#
.SYNOPSIS
    Zencoder OAuth client_credentials token helper for IronClaw (PowerShell).

.DESCRIPTION
    IronClaw's `ironclaw tool auth <name>` supports two flows:
      1. OAuth authorization_code + PKCE (browser redirect)
      2. Manual "paste the token" prompt
    Zencoder's token endpoint only supports client_credentials — a server-to-
    server grant with no browser redirect — so IronClaw cannot perform the
    exchange itself.

    This script does the exchange, prints the resulting JWT, and tells you
    exactly how to install it via `ironclaw tool auth zencoder-tool`.

.PARAMETER ClientId
    Zencoder Client ID. Prompted interactively if omitted.

.PARAMETER ClientSecret
    Zencoder Client Secret as SecureString. Prompted (hidden) if omitted.

.PARAMETER TokenUrl
    OAuth token endpoint. Default: https://fe.zencoder.ai/oauth/token.

.EXAMPLE
    .\zencoder-auth.ps1
    .\zencoder-auth.ps1 -ClientId "a936..." -TokenUrl "https://fe.zencoder.ai/oauth/token"
#>

[CmdletBinding()]
param(
    [string]       $ClientId,
    [SecureString] $ClientSecret,
    [string]       $TokenUrl = 'https://fe.zencoder.ai/oauth/token'
)

$ErrorActionPreference = 'Stop'

if (-not $ClientId) {
    $ClientId = Read-Host -Prompt 'Zencoder Client ID'
}
if ([string]::IsNullOrEmpty($ClientId)) {
    Write-Error 'Empty Client ID.'
    exit 2
}

if (-not $ClientSecret) {
    $ClientSecret = Read-Host -Prompt 'Zencoder Client Secret' -AsSecureString
}
$secretPlain = [System.Net.NetworkCredential]::new('', $ClientSecret).Password
if ([string]::IsNullOrEmpty($secretPlain)) {
    Write-Error 'Empty Client Secret.'
    exit 2
}

$body = @{
    client_id     = $ClientId
    client_secret = $secretPlain
    grant_type    = 'client_credentials'
} | ConvertTo-Json -Compress

try {
    $resp = Invoke-RestMethod `
        -Uri $TokenUrl `
        -Method Post `
        -ContentType 'application/json' `
        -Body $body
} catch {
    $status = if ($_.Exception.Response) { [int]$_.Exception.Response.StatusCode } else { 0 }
    Write-Error ("Token endpoint failed (HTTP {0}): {1}" -f $status, $_.Exception.Message)
    exit 3
}

if (-not $resp.access_token) {
    Write-Error ("Response did not contain access_token. Body: {0}" -f ($resp | ConvertTo-Json -Compress))
    exit 4
}

$token = [string]$resp.access_token

Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════════╗"
Write-Host "║  ✓ JWT obtained successfully                                 ║"
Write-Host "╚══════════════════════════════════════════════════════════════╝"
Write-Host ""
if ($resp.expires_in) {
    Write-Host ("  Token lifetime: {0}s (re-run this script to rotate)" -f $resp.expires_in)
    Write-Host ""
}
Write-Host "  Your Zencoder access token:"
Write-Host ""
Write-Output $token
Write-Host ""
Write-Host "  ─────────────────────────────────────────────────────────────"
Write-Host "  Next step — paste it into IronClaw:"
Write-Host ""
Write-Host "    ironclaw tool auth zencoder-tool"
Write-Host ""
Write-Host "  At the prompt:"
Write-Host "    * Press 's' to skip the browser-open step (headless / no browser)"
Write-Host "    * Paste the token above when asked 'Paste your token:'"
Write-Host "    * IronClaw will validate it against the Zencoder API"
Write-Host "  ─────────────────────────────────────────────────────────────"
