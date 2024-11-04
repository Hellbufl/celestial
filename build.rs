fn main() {
    println!("cargo:rustc-cdylib-link-arg=/DEF:exports.def");
}