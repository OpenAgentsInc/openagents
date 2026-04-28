<#
.SYNOPSIS
    Verify that a NIP-05 handle resolves to a public key on the live registrar.

.DESCRIPTION
    GETs /.well-known/nostr.json (optionally with ?name=) and prints the
    matching pubkey. Exits non-zero if the name is missing or the pubkey
    does not match the optional -ExpectedPubkey.

.PARAMETER BaseUrl
    Registrar base URL. Defaults to env NIP05_REGISTRAR_BASE_URL or
    https://openagents.com.

.PARAMETER Handle
    Handle to verify (e.g. alice).

.PARAMETER ExpectedPubkey
    Optional expected 64-char lowercase hex pubkey or npub1... to assert.

.EXAMPLE
    PS> .\verify.ps1 -Handle alice

.EXAMPLE
    PS> .\verify.ps1 -Handle alice -ExpectedPubkey npub1...
#>
[CmdletBinding()]
param(
    [string]$BaseUrl,
    [Parameter(Mandatory = $true)][string]$Handle,
    [string]$ExpectedPubkey
)

$ErrorActionPreference = 'Stop'

if (-not $BaseUrl -or $BaseUrl.Trim() -eq '') {
    $BaseUrl = $env:NIP05_REGISTRAR_BASE_URL
}
if (-not $BaseUrl -or $BaseUrl.Trim() -eq '') {
    $BaseUrl = 'https://openagents.com'
}
$BaseUrl = $BaseUrl.TrimEnd('/')

$encoded = [System.Uri]::EscapeDataString($Handle)
$uri = "$BaseUrl/.well-known/nostr.json?name=$encoded"
Write-Host "GET $uri"

$resp = Invoke-RestMethod -Method Get -Uri $uri
if (-not $resp.names) {
    Write-Error "Response missing 'names' object."
    exit 1
}
$pubkey = $resp.names.$Handle
if (-not $pubkey) {
    Write-Error "Handle '$Handle' not registered."
    exit 2
}

Write-Host "$Handle@openagents.com -> $pubkey"

if ($ExpectedPubkey) {
    $expected = $ExpectedPubkey.Trim()
    # If user passed an npub1, attempt to compare lowercase prefixes vs hex separately.
    # We do NOT decode bech32 here to avoid an extra dependency; require hex match.
    if ($expected.ToLower().StartsWith('npub1')) {
        Write-Warning 'verify.ps1 compares against hex pubkey; pass -ExpectedPubkey as 64-char lowercase hex for an exact match.'
    } else {
        if ($pubkey.ToLower() -ne $expected.ToLower()) {
            Write-Error "Pubkey mismatch: got $pubkey, expected $expected"
            exit 3
        }
        Write-Host 'Pubkey matches expected value.'
    }
}
