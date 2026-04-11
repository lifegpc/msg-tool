fn main() {
    let source_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let crypt_json_path = source_dir.join("crypt.json");
    let outdir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let level = std::env::var("MSG_TOOL_KIRIKIRI_ARC_GEN_LEVEL").unwrap_or("22".to_string());
    println!("cargo:rerun-if-env-changed=OUT_DIR");
    println!("cargo:rerun-if-changed={}", crypt_json_path.display());
    println!(
        "cargo:rerun-if-changed={}",
        source_dir.join("cx_cb").display()
    );
    let arc_level = level
        .parse::<i32>()
        .expect("MSG_TOOL_KIRIKIRI_ARC_GEN_LEVEL must be a valid integer");
    println!("cargo:rerun-if-env-changed=MSG_TOOL_KIRIKIRI_ARC_GEN_LEVEL");
    msg_tool_build::kr_arc::gen_cx_cb(&crypt_json_path, &outdir, arc_level).unwrap();
    let level = std::env::var("MSG_TOOL_KIRIKIRI_CRYPT_COMPRESS_LEVEL").unwrap_or("22".to_string());
    let level = level
        .parse::<i32>()
        .expect("MSG_TOOL_KIRIKIRI_CRYPT_COMPRESS_LEVEL must be a valid integer");
    println!("cargo:rerun-if-env-changed=MSG_TOOL_KIRIKIRI_CRYPT_COMPRESS_LEVEL");
    msg_tool_build::kr_arc::gen_crypt(&crypt_json_path, &outdir, level).unwrap();
    println!(
        "cargo:rerun-if-changed={}",
        source_dir.join("name_list").display()
    );
    msg_tool_build::kr_arc::gen_name_list(&crypt_json_path, &outdir, arc_level).unwrap();
}
