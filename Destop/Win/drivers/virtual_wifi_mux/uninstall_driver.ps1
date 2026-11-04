# Clean Rollback & Removal Script for gigachadmux.sys
Write-Host "Removing WiFi GigaChad 420 NDIS MUX Virtual Driver (if installed)..."
pnputil /delete-driver gigachadmux.inf /uninstall /force 2>$null
Write-Host "Rollback complete. Physical network adapters were not modified."

