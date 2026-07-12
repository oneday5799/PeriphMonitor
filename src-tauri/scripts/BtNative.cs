using System;
using System.Runtime.InteropServices;

[StructLayout(LayoutKind.Sequential, Size = 40)]
public struct BLUETOOTH_DEVICE_SEARCH_PARAMS {
    public uint dwSize;
    [MarshalAs(UnmanagedType.Bool)] public bool fReturnAuthenticated;
    [MarshalAs(UnmanagedType.Bool)] public bool fReturnRemembered;
    [MarshalAs(UnmanagedType.Bool)] public bool fReturnUnknown;
    [MarshalAs(UnmanagedType.Bool)] public bool fReturnConnected;
    [MarshalAs(UnmanagedType.Bool)] public bool fIssueInquiry;
    public byte cTimeoutMultiplier;
    public IntPtr hRadio;
}

[StructLayout(LayoutKind.Sequential, Size = 560)]
public struct BLUETOOTH_DEVICE_INFO {
    public uint dwSize;
    public BLUETOOTH_ADDRESS Address;
    public uint ulClassofDevice;
    [MarshalAs(UnmanagedType.Bool)] public bool fConnected;
    [MarshalAs(UnmanagedType.Bool)] public bool fRemembered;
    [MarshalAs(UnmanagedType.Bool)] public bool fAuthenticated;
    public long stLastSeen;
    public long stLastUsed;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 248)]
    public string szName;
}

[StructLayout(LayoutKind.Sequential)]
public struct BLUETOOTH_ADDRESS {
    public ulong ullLong;
}

[StructLayout(LayoutKind.Sequential)]
public struct BLUETOOTH_FIND_RADIO_PARAMS {
    public uint dwSize;
}

public class BtNative {
    [DllImport("BluetoothApis.dll", SetLastError = true)]
    public static extern IntPtr BluetoothFindFirstDevice(
        ref BLUETOOTH_DEVICE_SEARCH_PARAMS searchParams,
        ref BLUETOOTH_DEVICE_INFO deviceInfo);

    [DllImport("BluetoothApis.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool BluetoothFindNextDevice(
        IntPtr hFind,
        ref BLUETOOTH_DEVICE_INFO deviceInfo);

    [DllImport("BluetoothApis.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool BluetoothFindDeviceClose(IntPtr hFind);

    [DllImport("BluetoothApis.dll", SetLastError = true)]
    public static extern uint BluetoothGetDeviceInfo(
        IntPtr hRadio,
        ref BLUETOOTH_DEVICE_INFO deviceInfo);

    [DllImport("BluetoothApis.dll", SetLastError = true)]
    public static extern uint BluetoothSetServiceState(
        IntPtr hRadio,
        ref BLUETOOTH_DEVICE_INFO deviceInfo,
        ref Guid serviceGuid,
        uint dwServiceFlags);

    [DllImport("BluetoothApis.dll", SetLastError = true)]
    public static extern uint BluetoothEnumerateInstalledServices(
        IntPtr hRadio,
        ref BLUETOOTH_DEVICE_INFO deviceInfo,
        ref uint pServiceCount,
        IntPtr pGuidServices);

    [DllImport("BluetoothApis.dll", SetLastError = true)]
    public static extern IntPtr BluetoothFindFirstRadio(
        ref BLUETOOTH_FIND_RADIO_PARAMS pSearchParams,
        ref IntPtr phRadio);

    [DllImport("BluetoothApis.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool BluetoothFindNextRadio(
        IntPtr hFind,
        ref IntPtr phRadio);

    [DllImport("BluetoothApis.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool BluetoothFindRadioClose(IntPtr hFind);

    [DllImport("kernel32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool CloseHandle(IntPtr hObject);
}
