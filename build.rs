use std::io;
use std::env;
use chrono::Local;

fn main() -> io::Result<()> {
    // Generate dynamic version based on build time or environment variable
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=APP_VERSION");
    
    let version = env::var("APP_VERSION").unwrap_or_else(|_| {
        let now = Local::now();
        now.format("v%Y.%m.%d-%H%M").to_string()
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
