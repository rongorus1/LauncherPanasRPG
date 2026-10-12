/// Operating system names as used by Mojang rules (`linux`, `windows`, `osx`) and their arch names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Platform {
    pub os: &'static str,
    pub arch: &'static str,
}

impl Platform {
    pub fn current() -> Self {
        let os = match std::env::consts::OS {
            "windows" => "windows",
            "macos" => "osx",
            _ => "linux",
        };
        let arch = match std::env::consts::ARCH {
            "x86" => "x86",
            "aarch64" => "arm64",
            "arm" => "arm32",
            _ => "x86_64",
        };
        Self { os, arch }
    }

    /// Key used by the Mojang Java runtime index.
    pub fn java_runtime_key(&self) -> Option<&'static str> {
        Some(match (self.os, self.arch) {
            ("linux", "x86_64") => "linux",
            ("linux", "x86") => "linux-i386",
            ("windows", "x86_64") => "windows-x64",
            ("windows", "x86") => "windows-x86",
            ("windows", "arm64") => "windows-arm64",
            ("osx", "x86_64") => "mac-os",
            ("osx", "arm64") => "mac-os-arm64",
            _ => return None,
        })
    }

    pub fn is_64bit(&self) -> bool {
        matches!(self.arch, "x86_64" | "arm64")
    }
}
