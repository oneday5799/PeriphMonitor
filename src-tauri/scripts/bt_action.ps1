param([string]$Mac, [string]$Action, [string]$OutFile)

$script:macClean = $Mac.Replace(':','').ToUpper()
$script:log = @()
$script:log += "START action=$Action mac=$script:macClean"

$csPath = Join-Path $PSScriptRoot 'BtNative.cs'
$script:log += "CS_PATH:$csPath exists=$(Test-Path $csPath)"
Add-Type -Path $csPath

function Write-Log([string]$msg) {
    $script:log += $msg
}

function Save-AndExit([int]$code) {
    $script:log -join "`n" | Out-File -FilePath $OutFile -Encoding utf8
    exit $code
}

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

    Write-Log "SEARCHING on radio $Radio..."
    $hFind = [BtNative]::BluetoothFindFirstDevice([ref]$searchParams, [ref]$deviceInfo)
    if (-not $hFind) {
        $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
        Write-Log "FIND_FAILED:win32=$err"
        return $false
    }

    $found = $false
    $enumCount = 0
    do {
        $enumCount++
        $addrHex = $deviceInfo.Address.ullLong.ToString("X12")
        if ($addrHex -eq $TargetMac) {
            $found = $true
            break
        }
    } while ([BtNative]::BluetoothFindNextDevice($hFind, [ref]$deviceInfo))

    [BtNative]::BluetoothFindDeviceClose($hFind) | Out-Null
    Write-Log "ENUM_COUNT:$enumCount"

    if (-not $found) {
        Write-Log "NOT_ON_RADIO"
        return $false
    }

    Write-Log "FOUND:$($deviceInfo.szName) connected=$($deviceInfo.fConnected)"

    $svcCount = [uint32]32
    $guidSize = 16
    $bufferSize = $svcCount * $guidSize
    $buffer = [System.Runtime.InteropServices.Marshal]::AllocHGlobal($bufferSize)
    [System.Runtime.InteropServices.Marshal]::Copy((New-Object byte[] $bufferSize), 0, $buffer, $bufferSize)

    $r = [BtNative]::BluetoothEnumerateInstalledServices($Radio, [ref]$deviceInfo, [ref]$svcCount, $buffer)
    Write-Log "ENUM_RESULT:$r SVC_COUNT:$svcCount"

    $targetSvcs = @()
    for ($i = 0; $i -lt $svcCount; $i++) {
        $offset = $i * $guidSize
        $bytes = New-Object byte[] $guidSize
        [System.Runtime.InteropServices.Marshal]::Copy([IntPtr]::Add($buffer, $offset), $bytes, 0, $guidSize)
        $guid = New-Object Guid(,$bytes)
        $targetSvcs += $guid
        Write-Log "SVC[$i]:$guid"
    }

    [System.Runtime.InteropServices.Marshal]::FreeHGlobal($buffer)

    if ($targetSvcs.Count -eq 0) {
        $targetSvcs = @(
            [Guid]"0000110b-0000-1000-8000-00805f9b34fb"
            [Guid]"0000110c-0000-1000-8000-00805f9b34fb"
            [Guid]"0000110e-0000-1000-8000-00805f9b34fb"
            [Guid]"0000111e-0000-1000-8000-00805f9b34fb"
            [Guid]"0000111f-0000-1000-8000-00805f9b34fb"
            [Guid]"00001108-0000-1000-8000-00805f9b34fb"
        )
        Write-Log "USING_DEFAULT_SVCS"
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
                Write-Log "DIS:$svc -> $r (attempt $($retry+1))"
                if ($r -eq 0) { $ok = $true; break }
                Start-Sleep -Milliseconds 200
            }
            if ($ok) { $disabled++ }
            else { Write-Log "DIS_FAILED:$svc after $MAX_RETRY attempts" }
        }
        Write-Log "DISABLED:$disabled/$($targetSvcs.Count)"
    } elseif ($DoAction -eq "connect") {
        $disabled = 0
        foreach ($svc in $targetSvcs) {
            $r = [BtNative]::BluetoothSetServiceState($Radio, [ref]$deviceInfo, [ref]$svc, $DISABLE)
            if ($r -eq 0) { $disabled++ }
        }
        Write-Log "PRE_DISABLE:$disabled/$($targetSvcs.Count)"

        Start-Sleep -Milliseconds 500

        $enabled = 0
        foreach ($svc in $targetSvcs) {
            $ok = $false
            for ($retry = 0; $retry -lt $MAX_RETRY; $retry++) {
                $r = [BtNative]::BluetoothSetServiceState($Radio, [ref]$deviceInfo, [ref]$svc, $ENABLE)
                Write-Log "EN:$svc -> $r (attempt $($retry+1))"
                if ($r -eq 0) {
                    $ok = $true
                    $enabled++
                    Start-Sleep -Milliseconds 200
                    break
                }
                Start-Sleep -Milliseconds 300
            }
            if (-not $ok) {
                Write-Log "EN_FAILED:$svc after $MAX_RETRY attempts"
            }
        }
        Write-Log "ENABLED:$enabled/$($targetSvcs.Count)"
    }

    return $true
}

# --- Main ---
Write-Log "ENUM_RADIOS..."
$rParams = New-Object BLUETOOTH_FIND_RADIO_PARAMS
$rParams.dwSize = 4
$hRadio = [IntPtr]::Zero
$hRadioFind = [BtNative]::BluetoothFindFirstRadio([ref]$rParams, [ref]$hRadio)

if (-not $hRadioFind) {
    $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
    Write-Log "NO_RADIO:$err"
    Save-AndExit 1
}

Write-Log "RADIO_OK handle=$hRadio"

$deviceFound = $false

$deviceFound = Invoke-BtAction -Radio $hRadio -TargetMac $script:macClean -DoAction $Action
[BtNative]::CloseHandle($hRadio) | Out-Null

if (-not $deviceFound) {
    Write-Log "TRY_NEXT_RADIOS"
    $nextRadio = [IntPtr]::Zero
    while ([BtNative]::BluetoothFindNextRadio($hRadioFind, [ref]$nextRadio)) {
        Write-Log "RADIO_NEXT handle=$nextRadio"
        $deviceFound = Invoke-BtAction -Radio $nextRadio -TargetMac $script:macClean -DoAction $Action
        [BtNative]::CloseHandle($nextRadio) | Out-Null
        if ($deviceFound) { break }
    }
}

[BtNative]::BluetoothFindRadioClose($hRadioFind) | Out-Null

if (-not $deviceFound) {
    Write-Log "NOT_FOUND"
    Save-AndExit 1
}

Write-Log "DONE"
Save-AndExit 0
