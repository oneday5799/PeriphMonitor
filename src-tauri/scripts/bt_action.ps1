param([string]$Mac, [string]$Action)

$macClean = $Mac.Replace(':','').ToUpper()

$csPath = Join-Path $PSScriptRoot 'BtNative.cs'
Add-Type -Path $csPath

# Open first radio
$rParams = New-Object BLUETOOTH_FIND_RADIO_PARAMS
$rParams.dwSize = 4
$hRadio = [IntPtr]::Zero
$hRadioFind = [BtNative]::BluetoothFindFirstRadio([ref]$rParams, [ref]$hRadio)

if (-not $hRadioFind) {
    Write-Output "NO_RADIO"
    exit 1
}
[BtNative]::BluetoothFindRadioClose($hRadioFind) | Out-Null
Write-Output "RADIO_OK"

# Find the device
$searchParams = New-Object BLUETOOTH_DEVICE_SEARCH_PARAMS
$searchParams.dwSize = 40
$searchParams.fReturnAuthenticated = $true
$searchParams.fReturnRemembered = $true
$searchParams.fReturnConnected = $true
$searchParams.hRadio = $hRadio

$deviceInfo = New-Object BLUETOOTH_DEVICE_INFO
$deviceInfo.dwSize = 560

$hFind = [BtNative]::BluetoothFindFirstDevice([ref]$searchParams, [ref]$deviceInfo)

if (-not $hFind) {
    $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
    Write-Output "FIND_FAILED:$err"
    [BtNative]::CloseHandle($hRadio) | Out-Null
    exit 1
}

$found = $false
do {
    $addrHex = $deviceInfo.Address.ullLong.ToString("X12")
    if ($addrHex -eq $macClean) {
        $found = $true
        break
    }
} while ([BtNative]::BluetoothFindNextDevice($hFind, [ref]$deviceInfo))

[BtNative]::BluetoothFindDeviceClose($hFind) | Out-Null

if (-not $found) {
    Write-Output "NOT_FOUND"
    [BtNative]::CloseHandle($hRadio) | Out-Null
    exit 1
}

Write-Output "FOUND:$($deviceInfo.szName) connected=$($deviceInfo.fConnected)"

# Get installed services using IntPtr
$svcCount = [uint32]32
$guidSize = 16  # Size of GUID in bytes
$bufferSize = $svcCount * $guidSize
$buffer = [System.Runtime.InteropServices.Marshal]::AllocHGlobal($bufferSize)

# Zero the buffer
[System.Runtime.InteropServices.Marshal]::Copy((New-Object byte[] $bufferSize), 0, $buffer, $bufferSize)

# Call BluetoothEnumerateInstalledServices
$r = [BtNative]::BluetoothEnumerateInstalledServices($hRadio, [ref]$deviceInfo, [ref]$svcCount, $buffer)
Write-Output "ENUM_RESULT:$r SVC_COUNT:$svcCount"

# Read GUIDs from buffer
$targetSvcs = @()
for ($i = 0; $i -lt $svcCount; $i++) {
    $offset = $i * $guidSize
    $bytes = New-Object byte[] $guidSize
    [System.Runtime.InteropServices.Marshal]::Copy([IntPtr]::Add($buffer, $offset), $bytes, 0, $guidSize)
    $guid = New-Object Guid(,$bytes)
    $targetSvcs += $guid
    Write-Output "SVC[$i]:$guid"
}

[System.Runtime.InteropServices.Marshal]::FreeHGlobal($buffer)

# Fallback: if no services found, use common audio service GUIDs
if ($targetSvcs.Count -eq 0) {
    $targetSvcs = @(
        [Guid]"0000110b-0000-1000-8000-00805f9b34fb"  # A2DP Sink
        [Guid]"0000110c-0000-1000-8000-00805f9b34fb"  # A/V Remote Control
        [Guid]"0000110e-0000-1000-8000-00805f9b34fb"  # A/V Remote Control Controller
        [Guid]"0000111e-0000-1000-8000-00805f9b34fb"  # Handsfree
        [Guid]"0000111f-0000-1000-8000-00805f9b34fb"  # Handsfree Audio Gateway
        [Guid]"00001108-0000-1000-8000-00805f9b34fb"  # Headset
    )
    Write-Output "USING_DEFAULT_SVCS"
}

$DISABLE = [uint32]0
$ENABLE = [uint32]1

if ($Action -eq "disconnect") {
    $disabled = 0
    foreach ($svc in $targetSvcs) {
        $r = [BtNative]::BluetoothSetServiceState($hRadio, [ref]$deviceInfo, [ref]$svc, $DISABLE)
        Write-Output "DIS:$svc -> $r"
        if ($r -eq 0) { $disabled++ }
    }
    Write-Output "DISABLED:$disabled"
} elseif ($Action -eq "connect") {
    $enabled = 0
    foreach ($svc in $targetSvcs) {
        [BtNative]::BluetoothSetServiceState($hRadio, [ref]$deviceInfo, [ref]$svc, $DISABLE) | Out-Null
        Start-Sleep -Milliseconds 150
        $r = [BtNative]::BluetoothSetServiceState($hRadio, [ref]$deviceInfo, [ref]$svc, $ENABLE)
        Write-Output "EN:$svc -> $r"
        if ($r -eq 0) { $enabled++ }
        Start-Sleep -Milliseconds 1200
    }
    Write-Output "ENABLED:$enabled"
}

[BtNative]::CloseHandle($hRadio) | Out-Null
Write-Output "DONE"
