use std::{error::Error, ffi::CStr, fs::read_to_string, mem::MaybeUninit};

use byte_unit::Byte;

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
pub struct SystemInfo {
    pub distro_name: String,
    pub distro_id: String,
    pub distro_build_id: String,
    pub username: String,
    pub hostname: String,
    pub shell: String,
    pub kernel: String,
    pub total_mem: String,
    pub cached_mem: String,
    pub available_mem: String,
    pub used_mem: String,
}

/// Type of information to obtain.
#[derive(PartialEq)]
enum Type {
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
    for line in meminfo.lines() {
        if !line.starts_with(key) {
            // Doesn't have the key we are looking for.
            continue;
        }

        // Trim to get rid of the repeated whitespaces, making parsing easier.
        let line = line.trim();
        return Some(line.split_whitespace().nth(1)?.to_owned());
    }

    None
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

/// Returns the active kernel version.
fn get_by_type(r#type: Type) -> Result<String, Box<dyn Error>> {
    // Create an uninitialized instance of `utsname`.
    let mut info = unsafe { MaybeUninit::<libc::utsname>::zeroed().assume_init() };
    // Store the output of `uname` into `info` as long as the type isn't `Username`.
    if r#type != Type::Username {
        unsafe { libc::uname(&mut info as *mut _) };
    } else {
        // Drop `info` to free its resources asap, since it won't be used.
        drop(info)
    }

    let result;
    match r#type {
        Type::Username => unsafe {
            return Ok(CStr::from_ptr(libc::getlogin()).to_str()?.to_owned());
        },
        Type::HostName => {
            result = bytes_to_u8!(info.nodename);
        }
        Type::KernelVersion => {
            result = bytes_to_u8!(info.release);
        }
    }

    Ok(String::from_utf8(result)?.to_owned())
}

/// Fetches system information.
/// This can panic if it fails fetching properly.
pub fn get_system_information() -> SystemInfo {
    let os_release =
        read_to_string("/etc/os-release").expect("[ERROR] Failed reading /etc/os-release!");
    let meminfo = read_to_string("/proc/meminfo").expect("[ERROR] Failed reading /proc/meminfo!");
    let distro_name =
        parse_osr_key(&os_release, "NAME").expect("[ERROR] Failed reading os-release NAME!");
    let distro_id =
        parse_osr_key(&os_release, "ID").expect("[ERROR] Failed reading os-release ID!");
    let distro_build_id =
        parse_osr_key(&os_release, "BUILD_ID").expect("[ERROR] Failed reading os-releae BUILD_ID!");

    let username = get_by_type(Type::Username).expect("[ERROR] Failed fetching username!");
    let hostname = get_by_type(Type::HostName).expect("[ERROR] Failed fetching hostname!");
    let shell = env!("SHELL")
        .split('/')
        .last()
        .expect("[ERROR] Failed fetching the current shell!")
        .to_owned();
    let kernel = get_by_type(Type::KernelVersion).expect("[ERROR] Failed fetching kernel!");

    let total_mem = minf_get_gb(&meminfo, "MemTotal");
    let cached_mem = minf_get_gb(&meminfo, "Cached");
    let available_mem = minf_get_gb(&meminfo, "MemAvailable");

    let total_kb: f64 = parse_minf_key(&meminfo, "MemTotal")
        .unwrap()
        .parse()
        .unwrap();
    let free_kb: f64 = parse_minf_key(&meminfo, "MemAvailable")
        .unwrap()
        .parse()
        .unwrap();
    let used_mem = kb_to_gb(total_kb - free_kb);

    SystemInfo {
        distro_name,
        distro_id,
        distro_build_id,
        username,
        hostname,
        shell,
        kernel,
        total_mem,
        cached_mem,
        available_mem,
        used_mem,
    }
}
