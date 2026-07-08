use crate::device::DevType;
use crate::config;

pub fn classify_device(name: &str, pnp_class: &str, pnp_id: &str, caption: &str) -> DevType {
    let lower_combined = format!("{} {}", name, caption).to_lowercase();

    // Check for 2.4G wireless devices by VID/PID
    if pnp_id.starts_with("USB\\") && is_wireless_24g_by_vid_pid(pnp_id) {
        return DevType::Wireless24G;
    }

    if pnp_class.eq_ignore_ascii_case("AudioEndpoint") || pnp_class.eq_ignore_ascii_case("MEDIA") {
        return DevType::Audio;
    }
    if pnp_class.eq_ignore_ascii_case("Keyboard") || pnp_class.eq_ignore_ascii_case("Mouse") {
        return DevType::Usb;
    }
    if pnp_class.eq_ignore_ascii_case("Monitor") {
        return DevType::Monitor;
    }
    if pnp_class.eq_ignore_ascii_case("Bluetooth")
        || pnp_id.starts_with("BTHENUM\\")
        || pnp_id.starts_with("SWD\\")
    {
        if is_audio(&lower_combined) { return DevType::Audio; }
        if is_usb(name, caption) { return DevType::Usb; }
        return DevType::Other;
    }
    if pnp_class.eq_ignore_ascii_case("HIDClass") {
        if is_audio(&lower_combined) { return DevType::Audio; }
        if is_usb(name, caption) { return DevType::Usb; }
        return DevType::Other;
    }
    if pnp_id.starts_with("USB\\") && is_usb(name, caption) {
        return DevType::Usb;
    }
    DevType::Other
}

pub fn classify_bluetooth(name: &str) -> Option<DevType> {
    // MAC-address-only BLE devices (e.g. "Bluetooth e0:cc:f8:7f:d9:eb")
    if name.starts_with("Bluetooth ") && name.len() == 27 && name.as_bytes()[12] == b':' {
        if config::with_config(|c| c.show_unnamed_bt) {
            return Some(DevType::Other);
        }
        return None;
    }
    let lower = name.to_lowercase();
    if is_audio(&lower) { return Some(DevType::Audio); }
    if is_usb(name, "") { return Some(DevType::Usb); }
    Some(DevType::Other)
}

pub fn is_wireless_24g_by_vid_pid(pnp_id: &str) -> bool {
    let upper = pnp_id.to_uppercase();
    let vid = match upper.find("VID_") {
        Some(pos) => {
            let start = pos + 4;
            if start + 4 <= upper.len() { &upper[start..start + 4] } else { return false; }
        }
        None => return false,
    };
    let pid = match upper.find("PID_") {
        Some(pos) => {
            let start = pos + 4;
            if start + 4 <= upper.len() { &upper[start..start + 4] } else { return false; }
        }
        None => return false,
    };

    match vid {
        "046D" => matches!(pid, "C52B" | "C539" | "C53A" | "C53F" | "C548" | "C540"),
        "1532" => matches!(pid,
            "0064" | "0065" | "0066" | "0067" | "0068" | "0069" | "006A" | "006B" | "006C" | "006D" |
            "006E" | "006F" | "0070" | "0071" | "0072" | "0073" | "0074" | "0075" | "0076" | "0077" |
            "0078" | "0079" | "007A" | "007B" | "007C"
        ),
        "1038" => matches!(pid, "1122" | "1123" | "1124" | "1125" | "1126" | "1137" | "1138" | "1139" | "113A" | "113B"),
        "1B1C" => matches!(pid, "1A01" | "1A02" | "1A03" | "1A04" | "1A05" | "1A06" | "1A07" | "1A08" | "1A09" | "1A0A"),
        _ => false,
    }
}

pub fn is_audio(n: &str) -> bool {
    let l = n.to_lowercase();
    [
        "headphone", "headset", "earphone", "earbuds", "speaker", "耳机", "音箱", "扬声器",
        "音响", "airpods", "hifi", "dac", "amp", "glasses", "眼镜",
    ]
    .iter()
    .any(|k| l.contains(k))
}

pub fn is_usb(n: &str, c: &str) -> bool {
    let t = format!("{} {}", n, c).to_lowercase();
    [
        "mouse", "keyboard", "controller", "gamepad", "鼠标", "键盘", "手柄", "xbox", "webcam",
        "logitech", "razer", "corsair", "keychron", "orochi", "deathadder", "viper",
        "gpro", "g pro", "basilisk", "naga", "blackwidow", "hunters", "kaira",
        "steelseries", "hyperx", "coolermaster", "roccat", "zte", "雷蛇", "罗技",
    ]
    .iter()
    .any(|k| t.contains(k))
}

pub fn is_bt_service(pnp_id: &str) -> bool {
    let upper = pnp_id.to_uppercase();
    upper.starts_with("BTHLEDEVICE\\{") || upper.starts_with("BTHENUM\\{")
}

pub fn is_generic_hid(pnp_id: &str) -> bool {
    let upper = pnp_id.to_uppercase();
    if upper.contains("&COL") {
        return true;
    }
    if upper.starts_with("USB\\") {
        return true;
    }
    if upper.starts_with("BTHLEDEVICE\\{") || upper.starts_with("BTHENUM\\{") {
        return true;
    }
    false
}

pub fn is_system_device(pnp_id: &str) -> bool {
    let upper = pnp_id.to_uppercase();
    upper.starts_with("BTH\\MS_")
}
