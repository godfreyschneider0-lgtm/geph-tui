use std::io;
#[cfg(windows)]
use winres::WindowsResource;

fn main() -> io::Result<()> {
    #[cfg(windows)]
    {
        let mut res = WindowsResource::new();
        // Embed icon only if the file exists; skip gracefully otherwise.
        if std::path::Path::new("src/logo-naked.ico").exists() {
            res.set_icon("src/logo-naked.ico");
        }
        res.compile()?;
    }

    Ok(())
}
