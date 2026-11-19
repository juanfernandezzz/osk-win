param(
    [string]$BinaryDir = (Join-Path $PSScriptRoot '..\target\release'),
    [string]$SignedHelper,
    [string]$OutputDir = (Join-Path $PSScriptRoot 'Output'),
    [string]$Compiler = 'ISCC.exe'
)
$ErrorActionPreference = 'Stop'
$BinaryDir = (Resolve-Path -LiteralPath $BinaryDir).Path
if (-not $SignedHelper) { $SignedHelper = Join-Path $BinaryDir 'osk-win-input.exe' }
$SignedHelper = (Resolve-Path -LiteralPath $SignedHelper).Path
$manifest = Join-Path $PSScriptRoot '..\Cargo.toml'
$metadata = & cargo metadata --locked --offline --no-deps --format-version 1 --manifest-path $manifest
if ($LASTEXITCODE -ne 0) { throw 'Cannot read the locked workspace version' }
$version = (($metadata | ConvertFrom-Json).packages | Where-Object name -eq 'osk-win').version
$exe = Join-Path $BinaryDir 'osk-win.exe'
$compiled = (& $exe --version | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $compiled -ne "osk-win $version") {
    throw "Expected osk-win $version, got '$compiled'"
}
$cert = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new((Join-Path $PSScriptRoot 'osk-win-input.cer'))
$signature = Get-AuthenticodeSignature -LiteralPath $SignedHelper
if ($signature.SignerCertificate.Thumbprint -ne $cert.Thumbprint) { throw 'Unexpected UIAccess signer' }
# The originally shipped, signed v0.3.0 helper also verifies by content on
# build machines that have not chosen to trust the optional UIAccess root.
$original = '51397024B6D8B9C00853180B29C5F9F8ABCADE1BA001B4736B67301553A114E1'
if ($signature.Status -ne 'Valid' -and (Get-FileHash -LiteralPath $SignedHelper).Hash -ne $original) {
    throw "Invalid helper signature: $($signature.Status)"
}
[void][IO.Directory]::CreateDirectory($OutputDir)
$OutputDir = (Resolve-Path -LiteralPath $OutputDir).Path
& $Compiler "/DAppVersion=$version" "/DBinaryDir=$BinaryDir" "/DSignedHelper=$SignedHelper" "/O$OutputDir" (Join-Path $PSScriptRoot 'osk-win.iss')
if ($LASTEXITCODE -ne 0) { throw "ISCC exited with $LASTEXITCODE" }
$setup = Get-Item -LiteralPath (Join-Path $OutputDir 'osk-win-setup.exe')
if ($setup.VersionInfo.FileVersion.Trim() -ne "$version.0") { throw 'Installer version mismatch' }
Get-FileHash -LiteralPath $setup.FullName
