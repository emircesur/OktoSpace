//! Embed the icon and version information in the Windows executable.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/oktospace.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/oktospace.ico");
        res.set("FileDescription", "OktoSpace");
        res.set("ProductName", "OktoSpace");
        res.set("CompanyName", "OktoSpace contributors");
        if let Err(err) = res.compile() {
            // A missing resource compiler shouldn't stop development builds.
            println!("cargo:warning=could not embed the Windows icon: {err}");
        }
    }
}
