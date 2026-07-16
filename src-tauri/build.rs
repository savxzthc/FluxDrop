fn main() {
    println!("cargo:rerun-if-env-changed=FLUXDROP_BUILD_FLAVOR");
    println!("cargo:rerun-if-env-changed=TAURI_SIGNING_PUBLIC_KEY");
    tauri_build::build();
}
