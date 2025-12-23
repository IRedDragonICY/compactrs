use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;

fn main() -> io::Result<()> {
    // Rerun triggers
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=icon.ico");
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=app.debug.manifest");
    println!("cargo:rerun-if-env-changed=APP_VERSION");

    // Generate dynamic version
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

    // Only compile resources on Windows
    #[cfg(target_os = "windows")]
    compile_resources()?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn compile_resources() -> io::Result<()> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    
    // Choose manifest based on profile
    let manifest = if profile == "release" {
        "app.manifest"
    } else {
        "app.debug.manifest"
    };

    // Get absolute paths
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let icon_path = manifest_dir.join("icon.ico");
    let manifest_path = manifest_dir.join(manifest);

    // Create the resource file content
    // Note: We use raw numbers instead of includes to avoid header file dependencies
    // RT_ICON = 3, RT_GROUP_ICON = 14, RT_MANIFEST = 24
    let rc_content = format!(
        r#"// Auto-generated resource file - CompactRS
// Icon resource (ID 1)
1 ICON "{}"

// Manifest resource (ID 1, type 24 = RT_MANIFEST)
1 24 "{}"
"#,
        icon_path.to_string_lossy().replace('\\', "\\\\"),
        manifest_path.to_string_lossy().replace('\\', "\\\\")
    );

    // Write the .rc file
    let rc_path = out_dir.join("resource.rc");
    fs::write(&rc_path, &rc_content)?;

    // Find rc.exe from Windows SDK
    let rc_exe = find_rc_exe().ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "Could not find rc.exe")
    })?;

    // Compile the resource file
    let res_path = out_dir.join("resource.res");
    let status = Command::new(&rc_exe)
        .arg("/nologo")
        .arg("/fo")
        .arg(&res_path)
        .arg(&rc_path)
        .status()?;

    if !status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("rc.exe failed with exit code: {:?}", status.code()),
        ));
    }

    // Link the resource file
    println!("cargo:rustc-link-arg-bins={}", res_path.to_string_lossy());

    // Explicitly set entry point and subsystem for no_main
    println!("cargo:rustc-link-arg=/ENTRY:WinMainCRTStartup");
    println!("cargo:rustc-link-arg=/SUBSYSTEM:WINDOWS");
    
    // Explicitly link CRT libraries for symbols like memset
    #[cfg(target_env = "msvc")]
    {
        println!("cargo:rustc-link-lib=vcruntime");
        println!("cargo:rustc-link-lib=ucrt");
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn find_rc_exe() -> Option<PathBuf> {
    // Try common Windows SDK locations
    let program_files_x86 = env::var("ProgramFiles(x86)").unwrap_or_else(|_| "C:\\Program Files (x86)".to_string());
    let sdk_base = PathBuf::from(program_files_x86).join("Windows Kits\\10\\bin");

    if sdk_base.exists() {
        // Find the latest SDK version
        if let Ok(entries) = fs::read_dir(&sdk_base) {
            let mut versions: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter(|e| e.file_name().to_string_lossy().starts_with("10."))
                .collect();
            
            versions.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
            
            for version in versions {
                let rc_path = version.path().join("x64\\rc.exe");
                if rc_path.exists() {
                    return Some(rc_path);
                }
            }
        }
    }

    // Fallback: try PATH
    if Command::new("rc.exe").arg("/?").output().is_ok() {
        return Some(PathBuf::from("rc.exe"));
    }

    None
}
