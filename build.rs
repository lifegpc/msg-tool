fn main() {
    #[cfg(windows)]
    let default_stack_size = "4194304"; // 4 MiB
    #[cfg(not(windows))]
    let default_stack_size = "8388608"; // 8 MiB
    let stack_size = std::env::var("MSG_TOOL_STACK_SIZE").unwrap_or(default_stack_size.to_string());
    let stack_size = parse_size::parse_size(stack_size).unwrap();
    println!("cargo:rerun-if-env-changed=MSG_TOOL_STACK_SIZE");
    #[cfg(target_env = "msvc")]
    println!("cargo:rustc-link-arg=/STACK:{}", stack_size);
    #[cfg(target_env = "gnu")]
    println!("cargo:rustc-link-arg=-Wl,-z,stack-size={}", stack_size);
    #[cfg(feature = "kirikiri-arc")]
    {
        let source_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let crypt_json_path = source_dir.join("src/scripts/kirikiri/archive/xp3/crypt.json");
        let outdir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
        let level = std::env::var("MSG_TOOL_KIRIKIRI_ARC_GEN_LEVEL").unwrap_or("22".to_string());
        println!("cargo:rerun-if-env-changed=OUT_DIR");
        println!("cargo:rerun-if-changed={}", crypt_json_path.display());
        let level = level
            .parse::<i32>()
            .expect("MSG_TOOL_KIRIKIRI_ARC_GEN_LEVEL must be a valid integer");
        println!("cargo:rerun-if-env-changed=MSG_TOOL_KIRIKIRI_ARC_GEN_LEVEL");
        msg_tool_build::kr_arc::gen_cx_cb(&crypt_json_path, &outdir, level).unwrap();
    }
}
