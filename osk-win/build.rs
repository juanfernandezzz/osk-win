fn main() {
    // osk-win-input carries UIAccess, so Windows lets its keys reach apps
    // run as administrator (it must be signed and live in Program Files).
    println!("cargo:rustc-link-arg-bin=osk-win-input=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg-bin=osk-win-input=/MANIFESTUAC:level='asInvoker' uiAccess='true'");
}
