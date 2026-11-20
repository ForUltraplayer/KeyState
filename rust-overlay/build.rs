fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon_with_id("assets/icon.ico", "1");
        res.set("ProductName", "KeyState-RS");
        res.set("FileDescription", "KeyState-RS GPU Keyboard & Mouse Overlay");
        res.compile().expect("Failed to compile Windows icon resource");
    }
}
