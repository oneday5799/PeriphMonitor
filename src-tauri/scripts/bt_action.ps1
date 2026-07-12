param([string]$Mac, [string]$Action)

$macClean = $Mac.Replace(':','').ToUpper()

$csPath = Join-Path $PSScriptRoot 'BtNative.cs'
Add-Type -Path $csPath

# Helper: try action on one radio, return $true if device found on this radio
function Invoke-BtAction {
    param([IntPtr]$Radio, [string]$TargetMac, [string]$DoAction)

    $searchParams = New-Object BLUETOOTH_DEVICE_SEARCH_PARAMS
    $searchParams.dwSize = 40
    $searchParams.fReturnAuthenticated = $true
    $searchParams.fReturnRemembered = $true
    $searchParams.fReturnConnected = $true
    $searchParams.hRadio = $Radio

    $deviceInfo = New-Object BLUETOOTH_DEVICE_INFO
    $deviceInfo.dwSize = 560

    $hFind = [BtNative]::BluetoothFindFirstDevice([ref]$searchParams, [ref]$deviceInfo)
    if (-not $hFind) {
        $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
        Write-Verbose "  FindFirstDevice failed: $err"
        return $false
    }

    $found = $false
    do {
        $addrHex = $deviceInfo.Address.ullLong.ToString("X12")
        if ($addrHex -eq $TargetMac) {
            $found = $true
            break
        }
    } while ([BtNative]::BluetoothFindNextDevice($hFind, [ref]$deviceInfo))

    [BtNative]::BluetoothFindDeviceClose($hFind) | Out-Null

    if (-not $found) {
        Write-Verbose "  Device not found on this radio"
        return $false
    }

    Write-Output "FOUND:$($deviceInfo.szName) connected=$($deviceInfo.fConnected)"

    # Enumerate installed services
    $svcCount = [uint32]32
    $guidSize = 16
    $bufferSize = $svcCount * $guidSize
    $buffer = [System.Runtime.InteropServices.Marshal]::AllocHGlobal($bufferSize)
    [System.Runtime.InteropServices.Marshal]::Copy((New-Object byte[] $bufferSize), 0, $buffer, $bufferSize)

    $r = [BtNative]::BluetoothEnumerateInstalledServices($Radio, [ref]$deviceInfo, [ref]$svcCount, $buffer)
    Write-Output "ENUM_RESULT:$r SVC_COUNT:$svcCount"

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
    $MAX_RETRY = 3

    if ($DoAction -eq "disconnect") {
        $disabled = 0
        foreach ($svc in $targetSvcs) {
            $ok = $false
            for ($retry = 0; $retry -lt $MAX_RETRY; $retry++) {
                $r = [BtNative]::BluetoothSetServiceState($Radio, [ref]$deviceInfo, [ref]$svc, $DISABLE)
                Write-Output "DIS:$svc -> $r (attempt $($retry+1))"
                if ($r -eq 0) { $ok = $true; break }
                Start-Sleep -Milliseconds 200
            }
            if ($ok) { $disabled++ }
            else { Write-Output "DIS_FAILED:$svc after $MAX_RETRY attempts" }
        }
        Write-Output "DISABLED:$disabled/$($targetSvcs.Count)"
    } elseif ($DoAction -eq "connect") {
        # Step 1: Disable all services first, then wait for stack to settle
        $disabled = 0
        foreach ($svc in $targetSvcs) {
            $r = [BtNative]::BluetoothSetServiceState($Radio, [ref]$deviceInfo, [ref]$svc, $DISABLE)
            if ($r -eq 0) { $disabled++ }
        }
        Write-Output "PRE_DISABLE:$disabled/$($targetSvcs.Count)"

        # Wait for Bluetooth stack to process disconnections
        Start-Sleep -Milliseconds 500

        # Step 2: Enable services with retry
        $enabled = 0
        foreach ($svc in $targetSvcs) {
            $ok = $false
            for ($retry = 0; $retry -lt $MAX_RETRY; $retry++) {
                $r = [BtNative]::BluetoothSetServiceState($Radio, [ref]$deviceInfo, [ref]$svc, $ENABLE)
                Write-Output "EN:$svc -> $r (attempt $($retry+1))"
                if ($r -eq 0) {
                    $ok = $true
                    $enabled++
                    Start-Sleep -Milliseconds 200
                    break
                }
                Start-Sleep -Milliseconds 300
            }
            if (-not $ok) {
                Write-Output "EN_FAILED:$svc after $MAX_RETRY attempts"
            }
        }
        Write-Output "ENABLED:$enabled/$($targetSvcs.Count)"
    }

    return $true
}

# --- Main: iterate all Bluetooth radios ---
$rParams = New-Object BLUETOOTH_FIND_RADIO_PARAMS
$rParams.dwSize = 4
$hRadio = [IntPtr]::Zero
$hRadioFind = [BtNative]::BluetoothFindFirstRadio([ref]$rParams, [ref]$hRadio)

if (-not $hRadioFind) {
    $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
    Write-Output "NO_RADIO:$err"
    exit 1
}

$deviceFound = $false

# Try first radio
$deviceFound = Invoke-BtAction -Radio $hRadio -TargetMac $macClean -DoAction $Action
[BtNative]::CloseHandle($hRadio) | Out-Null

# If not found, try remaining radios
if (-not $deviceFound) {
    $nextRadio = [IntPtr]::Zero
    while ([BtNative]::BluetoothFindNextRadio($hRadioFind, [ref]$nextRadio)) {
        $deviceFound = Invoke-BtAction -Radio $nextRadio -TargetMac $macClean -DoAction $Action
        [BtNative]::CloseHandle($nextRadio) | Out-Null
        if ($deviceFound) { break }
    }
}

[BtNative]::BluetoothFindRadioClose($hRadioFind) | Out-Null

if (-not $deviceFound) {
    Write-Output "NOT_FOUND"
    exit 1
}

Write-Output "DONE"
