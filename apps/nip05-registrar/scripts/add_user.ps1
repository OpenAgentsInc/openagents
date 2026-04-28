<#
.SYNOPSIS
    Claim an OpenAgents NIP-05 handle by POSTing to the registrar.

.DESCRIPTION
    Sends a runtime-authenticated claim to the live NIP-05 registrar service.
    The bearer token must come from the booth operator's environment; do not
    hard-code it in source control.

.PARAMETER BaseUrl
    Registrar base URL (e.g. https://openagents.com). Defaults to env
    NIP05_REGISTRAR_BASE_URL or https://openagents.com.

.PARAMETER Token
    Operator bearer token. Defaults to env NIP05_REGISTRAR_ADMIN_TOKEN.

.PARAMETER Handle
    Handle to claim. Must match ^[a-z0-9_\-\.]{1,32}$.

.PARAMETER Npub
    npub1... bech32 OR 64-char lowercase hex public key.

.EXAMPLE
    PS> .\add_user.ps1 -Handle alice -Npub npub1...

.EXAMPLE
    PS> .\add_user.ps1 -BaseUrl https://staging.openagents.com `
                       -Handle bob -Npub abcd...64hex
#>
[CmdletBinding()]
param(
    [string]$BaseUrl,
    [string]$Token,
    [Parameter(Mandatory = $true)][string]$Handle,
    [Parameter(Mandatory = $true)][string]$Npub
)

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

$body = @{ name = $Handle }
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
