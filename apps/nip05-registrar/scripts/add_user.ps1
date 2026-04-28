<#
.SYNOPSIS
    Operator override: claim an OpenAgents NIP-05 handle without proof of
    Nostr key control.

.DESCRIPTION
    POSTs an operator-override claim to /admin/claim. This BYPASSES the
    normal /claim/challenge + /claim/complete proof-of-control flow and
    should only be used to:
      - Bootstrap officially-managed reserved handles on a fresh deploy
        (e.g. seeding 'agent' to the OpenAgents-controlled key).
      - Correct an emergency that cannot be resolved via the public flow.

    For users who already have a Nostr key, point them at
    https://openagents.com/claim (the self-serve OTP flow) instead.

    The bearer token must come from the booth operator's environment; do
    not hard-code it in source control. Each override claim is logged
    server-side as event=claim_admin_override for audit.

.PARAMETER BaseUrl
    Registrar base URL (e.g. https://openagents.com). Defaults to env
    NIP05_REGISTRAR_BASE_URL or https://openagents.com.

.PARAMETER Token
    Operator bearer token. Defaults to env NIP05_REGISTRAR_ADMIN_TOKEN.

.PARAMETER Handle
    Handle to claim. Must match ^[a-z0-9_\-\.]{1,32}$.

.PARAMETER Npub
    npub1... bech32 OR 64-char lowercase hex x-only public key.

.PARAMETER Confirm
    Required. Acknowledges the operator-override semantics. Pass
    -Confirm to actually run.

.EXAMPLE
    PS> .\add_user.ps1 -Handle agent -Npub npub1... -Confirm
#>
[CmdletBinding()]
param(
    [string]$BaseUrl,
    [string]$Token,
    [Parameter(Mandatory = $true)][string]$Handle,
    [Parameter(Mandatory = $true)][string]$Npub,
    [switch]$Confirm
)

if (-not $Confirm) {
    Write-Error "This script issues an OPERATOR OVERRIDE claim that bypasses Nostr key proof-of-control. Pass -Confirm to acknowledge. For self-service claims with proof, use https://openagents.com/claim instead."
    exit 2
}

$ErrorActionPreference = 'Stop'

if (-not $BaseUrl -or $BaseUrl.Trim() -eq '') {
    $BaseUrl = $env:NIP05_REGISTRAR_BASE_URL
}
if (-not $BaseUrl -or $BaseUrl.Trim() -eq '') {
    $BaseUrl = 'https://openagents.com'
}
$BaseUrl = $BaseUrl.TrimEnd('/')

if (-not $Token -or $Token.Trim() -eq '') {
    $Token = $env:NIP05_REGISTRAR_ADMIN_TOKEN
}
if (-not $Token -or $Token.Trim() -eq '') {
    throw 'Operator bearer token is required. Pass -Token or set NIP05_REGISTRAR_ADMIN_TOKEN.'
}

$body = @{ name = $Handle; operator_override = $true }
if ($Npub.ToLower().StartsWith('npub1')) {
    $body['npub'] = $Npub
} else {
    $body['pubkey'] = $Npub
}
$json = $body | ConvertTo-Json -Compress

$headers = @{
    Authorization  = "Bearer $Token"
    'Content-Type' = 'application/json'
}

$uri = "$BaseUrl/admin/claim"
Write-Host "POST $uri"
try {
    $resp = Invoke-RestMethod -Method Post -Uri $uri -Headers $headers -Body $json
    Write-Host "Claim successful:"
    $resp | ConvertTo-Json -Depth 4 | Write-Host
    Write-Host "Verify: $BaseUrl/.well-known/nostr.json?name=$Handle"
} catch {
    Write-Error "Claim failed: $($_.Exception.Message)"
    if ($_.ErrorDetails -and $_.ErrorDetails.Message) {
        Write-Error $_.ErrorDetails.Message
    }
    exit 1
}
