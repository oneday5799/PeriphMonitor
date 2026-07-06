// Copyright (c) ScaleFS LLC; used with permission
// Licensed under the MIT License

use windows_pnp_uuid::Uuid;

pub enum PnpDevicePropertyValue {
    ArrayOfValues(/*array: */Vec<PnpDevicePropertyValue>),
    Boolean(/*value: */bool),
    Byte(/*value: */u8),
    Guid(/*value: */Uuid),
    ListOfValues(/*list: */Vec<PnpDevicePropertyValue>),
    String(/*value: */String),
    UInt16(/*value: */u16),
    UInt32(/*value: */u32),
    UnsupportedPropertyDataType(/*property_data_type: DEVPROPTYPE*/u32),
    UnsupportedRegistryDataType(/*registry_data_type: REG_VALUE_TYPE*/u32),
}
