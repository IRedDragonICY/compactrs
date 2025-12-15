use std::io;
use std::env;

fn main() -> io::Result<()> {
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
