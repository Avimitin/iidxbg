fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/iidxbg.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // GPUI's Windows platform loads the application's icon from resource 1.
        embed_resource::compile("assets/app.rc", embed_resource::NONE)
            .manifest_required()
            .expect("compile the bundled Windows application icon");
    }
}
