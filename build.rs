use std::io;

fn main() -> io::Result<()> {
    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        // res.set_icon("icon.ico"); // If we had an icon
        res.set_manifest_file("app.manifest");
        res.compile()?;
    }
    Ok(())
}
