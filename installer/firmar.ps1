# Signs osk-win-input.exe for UIAccess with a fresh self-signed certificate,
# exports its public part to installer\osk-win-input.cer and deletes the
# private key, so nothing else can ever be signed with it. Run before ISCC.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
$exe = Join-Path $root 'target\release\osk-win-input.exe'
$cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject 'CN=osk-win input helper' `
    -CertStoreLocation Cert:\CurrentUser\My -NotAfter (Get-Date).AddYears(20)
try {
    $sig = Set-AuthenticodeSignature -FilePath $exe -Certificate $cert -HashAlgorithm SHA256
    if ($sig.Status -ne 'Valid' -and $sig.Status -ne 'UnknownError') { throw "firma: $($sig.Status) $($sig.StatusMessage)" }
    Export-Certificate -Cert $cert -FilePath (Join-Path $PSScriptRoot 'osk-win-input.cer') | Out-Null
} finally {
    Remove-Item "Cert:\CurrentUser\My\$($cert.Thumbprint)" -DeleteKey
    # Windows also drops a public copy among the user's intermediate CAs.
    Remove-Item "Cert:\CurrentUser\CA\$($cert.Thumbprint)" -ErrorAction SilentlyContinue
}
"firmado $exe con $($cert.Thumbprint)"
