<#
.SYNOPSIS
    Zencoder OAuth client_credentials helper for IronClaw (PowerShell).

.DESCRIPTION
    IronClaw's `ironclaw tool auth <name>` does NOT implement the OAuth2
    client_credentials grant — Zencoder's token endpoint only supports
    that grant — so this script does the exchange and installs the
    resulting JWT as the IronClaw secret `zencoder_access_token`.

.PARAMETER ClientId
    Zencoder Client ID. Prompted interactively if omitted.

.PARAMETER ClientSecret
    Zencoder Client Secret as SecureString. Prompted (hidden) if omitted.

.PARAMETER SecretName
    IronClaw secret to write. Default: zencoder_access_token.

.PARAMETER TokenUrl
    OAuth token endpoint. Default: https://fe.zencoder.ai/oauth/token.

.PARAMETER PrintOnly
    Print the JWT to stdout and DO NOT call `ironclaw secret set`.

.EXAMPLE
    .\zencoder-auth.ps1
    .\zencoder-auth.ps1 -PrintOnly
#>

[CmdletBinding()]
param(
    [string]       $ClientId,
    [SecureString] $ClientSecret,
    [string]       $SecretName = 'zencoder_access_token',
    [string]       $TokenUrl   = 'https://fe.zencoder.ai/oauth/token',
    [switch]       $PrintOnly
)

$ErrorActionPreference = 'Stop'

if (-not $PrintOnly) {
    if (-not (Get-Command ironclaw -ErrorAction SilentlyContinue)) {
        Write-Error "'ironclaw' not in PATH. Install it first or rerun with -PrintOnly."
        exit 1
    }
}

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

if ($PrintOnly) {
    Write-Output $token
    exit 0
}

# Try `ironclaw secret set <name> --stdin` first; fall back to positional.
$setOk = $false
try {
    $token | & ironclaw secret set $SecretName --stdin | Out-Null
    if ($LASTEXITCODE -eq 0) { $setOk = $true }
} catch { $setOk = $false }

if (-not $setOk) {
    Write-Warning "Falling back to positional 'ironclaw secret set <name> <token>'. The JWT will be briefly visible in the OS process list — upgrade IronClaw to a build supporting --stdin to avoid this."
    & ironclaw secret set $SecretName $token | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Error ("'ironclaw secret set $SecretName' failed. Token (set manually):`n{0}" -f $token)
        exit 5
    }
}

Write-Host ("OK: stored Zencoder JWT in IronClaw secret '{0}'." -f $SecretName)
if ($resp.expires_in) {
    Write-Host ("    Token lifetime: {0}s (rerun this script to rotate)." -f $resp.expires_in)
}
Write-Host '    Validate with: ironclaw tool auth zencoder-tool'
