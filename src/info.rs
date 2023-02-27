use byte_unit::Byte;
use std::{ffi::CStr, fs::read_to_string, mem::MaybeUninit};

/// Simple macro to convert all bytes to their u8 representation.
macro_rules! bytes_to_u8 {
    ($collection:expr) => {
        $collection
            .iter()
            .map(|byte| *byte as u8)
            .collect::<Vec<_>>()
    };
}

/// Fetched system information.
#[derive(Debug)]
pub struct SystemInfo {
    pub distro_name: String,
    pub distro_id: String,
    pub distro_build_id: String,
    pub username: String,
    pub hostname: String,
    pub shell: String,
    pub kernel: String,
    pub uptime_seconds: u32,
    pub uptime_minutes: u32,
    pub uptime_hours: u32,
    pub uptime_days: u32,
    pub uptime_formatted: String,
    pub total_mem: String,
    pub cached_mem: String,
    pub available_mem: String,
    pub used_mem: String,
}

/// Uptime structure.
struct Uptime {
    formatted: String,
    seconds: u32,
    minutes: u32,
    hours: u32,
    days: u32,
}

/// Type of information to obtain.
#[derive(PartialEq)]
pub enum Type {
    Username,
    HostName,
    KernelVersion,
}

/// Parses the given os-release key as a `String`.
fn parse_osr_key(os_release: &str, key: &str) -> Option<String> {
    let mut split = os_release.split(&format!("{key}=")).nth(1)?.to_owned();
    if split.contains('\n') {
        // Only get the first line from the result.
        split = split.split('\n').next()?.to_owned()
    }

    if split.contains('"') {
        // Don't keep double-quotes.
        split = split.replace('"', "")
    }

    Some(split)
}

/// Parses the given MemInfo key as a `String`.
fn parse_minf_key(meminfo: &str, key: &str) -> Option<String> {
    let line = meminfo.lines().find(|line| line.starts_with(key))?;
    Some(line.split_whitespace().nth(1)?.to_owned())
}

/// Converts the value of the given MemInfo key, into the gigabytes representation.
fn minf_get_gb(meminfo: &str, key: &str) -> String {
    let parsed: f64 = parse_minf_key(meminfo, key).unwrap().parse().unwrap();
    kb_to_gb(parsed)
}

/// Converts kilobytes to gigabytes.
fn kb_to_gb(number: f64) -> String {
    let unit = Byte::from_unit(number, byte_unit::ByteUnit::KB).unwrap();
    unit.get_adjusted_unit(byte_unit::ByteUnit::GB).to_string()
}

/// Fetches certan system info through `libc`.
pub fn get_by_type(r#type: Type) -> Option<String> {
    // Create an uninitialized instance of `utsname`.
    let mut info = unsafe { MaybeUninit::<libc::utsname>::zeroed().assume_init() };
    // Store the output of `uname` into `info` as long as the type isn't `Username`.
    if r#type != Type::Username {
        unsafe { libc::uname(&mut info as *mut _) };
    }

    let result = match r#type {
        Type::Username => unsafe {
            CStr::from_ptr(libc::getlogin())
                .to_str()
                .expect("[ERROR] Failed retrieving username!")
                .to_owned()
        },
        Type::HostName => String::from_utf8(bytes_to_u8!(info.nodename))
            .expect("[ERROR] Failed converting libc HostName output to a String!"),
        Type::KernelVersion => String::from_utf8(bytes_to_u8!(info.release))
            .expect("[ERROR] Failed converting libc KernelVersion output to a String!"),
    };

    Some(if result.contains('\0') {
        // Contains \0, split and get the content in front of it.
        result.split('\0').next()?.to_owned()
    } else {
        result
    })
}

/// Returns the uptime.
/// For example: `1 day, 1 hour, 20 minutes`
fn get_uptime() -> Uptime {
    let total_seconds =
        read_to_string("/proc/uptime").expect("[ERROR] Failed reading /proc/uptime!");
    let total_seconds: u32 = total_seconds
        .split('.')
        .next()
        .unwrap_or_default()
        .parse()
        .unwrap_or_default();
    let days = total_seconds / 86400;
    let hours = total_seconds / 3600;
    let minutes = total_seconds % 3600 / 60;
    let mut result = String::new();

    // Pretty-format it before returning
    if days > 0 {
        result.push_str(&days.to_string());
        result.push_str(if days > 1 { " days" } else { " day" });
        if hours > 0 {
            result.push(',')
        }
    }

    if hours > 0 {
        if days > 0 {
            result.push(' ');
        }

        result.push_str(&hours.to_string());
        result.push_str(if hours > 1 { " hours" } else { " hour" });
        if minutes > 0 {
            result.push(',')
        }
    }

    if minutes > 0 {
        if hours > 0 || days > 0 {
            result.push(' ')
        }

        result.push_str(&minutes.to_string());
        result.push_str(if minutes > 1 { " minutes" } else { " minute" });
    }

    // If the result is empty, then the system was most probably just powered on, so display the
    // seconds.
    if result.is_empty() {
        result = total_seconds.to_string();
        result.push_str(if total_seconds > 1 {
            " seconds"
        } else {
            " second"
        })
    }

    Uptime {
        formatted: result,
        seconds: total_seconds,
        minutes,
        hours,
        days,
    }
}

/// Fetches system information.
/// This can panic if it fails fetching properly.
pub fn get_system_information() -> Option<SystemInfo> {
    let os_release =
        read_to_string("/etc/os-release").expect("[ERROR] Failed reading /etc/os-release!");
    let meminfo = read_to_string("/proc/meminfo").expect("[ERROR] Failed reading /etc/meminfo!");
    let distro_name = parse_osr_key(&os_release, "NAME")?;
    let distro_id = parse_osr_key(&os_release, "ID")?;
    let distro_build_id = parse_osr_key(&os_release, "BUILD_ID")?;

    let username = get_by_type(Type::Username)?;
    let hostname = get_by_type(Type::HostName)?;
    let shell = std::env::var("SHELL").unwrap().split('/').last()?.to_owned();
    let kernel = get_by_type(Type::KernelVersion)?;

    let total_mem = minf_get_gb(&meminfo, "MemTotal");
    let cached_mem = minf_get_gb(&meminfo, "Cached");
    let available_mem = minf_get_gb(&meminfo, "MemAvailable");

    let total_kb: f64 = parse_minf_key(&meminfo, "MemTotal")?.parse().unwrap();
    let available_kb: f64 = parse_minf_key(&meminfo, "MemAvailable")?.parse().unwrap();
    let used_mem = kb_to_gb(total_kb - available_kb);

    let uptime = get_uptime();

    Some(SystemInfo {
        distro_name,
        distro_id,
        distro_build_id,
        username,
        hostname,
        shell,
        kernel,
        uptime_seconds: uptime.seconds,
        uptime_minutes: uptime.minutes,
        uptime_hours: uptime.hours,
        uptime_days: uptime.days,
        uptime_formatted: uptime.formatted,
        total_mem,
        cached_mem,
        available_mem,
        used_mem,
    })
}
