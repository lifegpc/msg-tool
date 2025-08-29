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
}
