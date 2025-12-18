use std::env;
use std::io;
use std::process::Command;

fn main() -> io::Result<()> {
    // Generate dynamic version based on build time or environment variable
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=APP_VERSION");

    let version = env::var("APP_VERSION").unwrap_or_else(|_| {
        let mut v = String::from("v0.0.0-local");
        if cfg!(target_os = "windows") {
            if let Ok(output) = Command::new("powershell")
                .args(["-NoProfile", "-Command", "Get-Date -Format 'vyyyy.MM.dd-HHmm'"])
                .output()
            {
                if output.status.success() {
                    if let Ok(s) = String::from_utf8(output.stdout) {
                        let trimmed = s.trim();
                        if !trimmed.is_empty() {
                            v = trimmed.to_string();
                        }
                    }
                }
            }
        }
        v
    });

    println!("cargo:rustc-env=APP_VERSION={}", version);

    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("icon.ico");

        // Use different manifest based on build profile
        // Release: requireAdministrator (app.manifest)
        // Debug: asInvoker (app.debug.manifest)
        let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
        let manifest = if profile == "release" {
            "app.manifest"
        } else {
            "app.debug.manifest"
        };

        res.set_manifest_file(manifest);
        res.compile()?;
    }
    Ok(())
}
