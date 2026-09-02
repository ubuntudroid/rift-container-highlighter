fn main() {
    // The vendored bindings call into SkyLight, a private framework.
    println!("cargo:rustc-link-search=framework=/System/Library/PrivateFrameworks");
    println!("cargo:rustc-link-lib=framework=SkyLight");
}
