// Copyright (c) ScaleFS LLC; used with permission
// Licensed under the MIT License

use windows_pnp_uuid::Uuid;
use windows_sys::Win32::Foundation::{DEVPROPKEY, PROPERTYKEY};

#[derive(PartialEq, Eq, Hash)]
pub struct PnpDevicePropertyKey {
    pub fmtid: Uuid,
    pub pid: u32,
}
impl PnpDevicePropertyKey {
    pub fn to_devpropkey(&self) -> DEVPROPKEY {
        DEVPROPKEY {
            fmtid: windows_sys::core::GUID { data1: self.fmtid.data1, data2: self.fmtid.data2, data3: self.fmtid.data3, data4: self.fmtid.data4 },
            pid: self.pid
        }
    }
}
impl From<DEVPROPKEY> for PnpDevicePropertyKey {
    fn from(item: DEVPROPKEY) -> Self {
        PnpDevicePropertyKey {
            fmtid: Uuid { data1: item.fmtid.data1, data2: item.fmtid.data2, data3: item.fmtid.data3, data4: item.fmtid.data4 },
            pid: item.pid
        }
    }
}
impl From<PROPERTYKEY> for PnpDevicePropertyKey {
    fn from(item: PROPERTYKEY) -> Self {
        PnpDevicePropertyKey {
            fmtid: Uuid { data1: item.fmtid.data1, data2: item.fmtid.data2, data3: item.fmtid.data3, data4: item.fmtid.data4 },
            pid: item.pid
        }
    }
}

