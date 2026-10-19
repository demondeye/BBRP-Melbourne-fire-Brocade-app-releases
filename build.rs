// Gives the Windows .exe its icon and file details.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico")
            .set("ProductName", "Melbourne Fire Brigade")
            .set("FileDescription", "Melbourne Fire Brigade")
            .set("CompanyName", "Melbourne Fire Brigade (fictional)");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed the Windows icon: {e}");
        }
    }
    println!("cargo:rerun-if-changed=assets/icon.ico");
}
