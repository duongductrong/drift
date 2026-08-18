//! Detects the macOS SDK we are building against.
//!
//! The macOS 26 SDK widened the gap between the traffic-light buttons, which
//! changes how much clearance the custom toolbar has to reserve for them. The
//! resulting `macos_sdk_26_or_later` cfg is consumed by `ui::title_bar`.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(macos_sdk_26_or_later)");

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let major_version = Command::new("xcrun")
            .args(["--sdk", "macosx", "--show-sdk-version"])
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|version| version.trim().split('.').next()?.parse::<u32>().ok());

        if major_version.is_some_and(|major| major >= 26) {
            println!("cargo:rustc-cfg=macos_sdk_26_or_later");
        }
    }
}
