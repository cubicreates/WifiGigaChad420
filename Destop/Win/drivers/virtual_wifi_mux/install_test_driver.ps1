# Opt-In Controlled Development VM Installation Script for gigachadmux.sys
# Per Rule 5: Never executed automatically by WifiGigaChad420.exe.
param([switch]$ConfirmTestEnvironment)

if (-not $ConfirmTestEnvironment) {
    Write-Warning "BLOCKED (Rule 5 Safety Guard): Pass -ConfirmTestEnvironment in a test-signed WDK VM to install gigachadmux.inf."
    exit 1
}

pnputil /add-driver "$PSScriptRoot\gigachadmux.inf" /install

