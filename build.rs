#[cfg(feature = "private")]
mod private {
    use serde::Deserialize;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    #[derive(Debug, Deserialize)]
    #[serde(untagged)]
    enum DependencyType {
        Git { git: String, commit: String },
    }
    fn default_features() -> bool {
        true
    }
    #[derive(Debug, Deserialize)]
    struct Dependency {
        #[serde(flatten)]
        dep: DependencyType,
        #[serde(default)]
        package: Option<String>,
        #[serde(default)]
        features: Vec<String>,
        #[serde(rename = "default-features", default = "default_features")]
        default_features: bool,
    }
    #[derive(Debug, Deserialize)]
    struct Manifest {
        dependencies: HashMap<String, Dependency>,
        features: HashMap<String, Vec<String>>,
    }
    fn is_feature_enabled(name: &str) -> bool {
        let key = format!("CARGO_FEATURE_{}", name.to_uppercase().replace('-', "_"));
        std::env::var(key).is_ok()
    }

    fn clone_and_checkout(dep_dir: &Path, git: &str, commit: &str) {
        if dep_dir.exists() {
            if dep_dir.join(".git").exists() {
                let output = Command::new("git")
                    .args(["rev-parse", "HEAD"])
                    .current_dir(dep_dir)
                    .output()
                    .expect("failed to run git rev-parse");
                if output.status.success() {
                    let current = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if current == commit {
                        return;
                    }
                }
            }
            std::fs::remove_dir_all(dep_dir).expect("failed to remove old dependency directory");
        }

        let parent = dep_dir.parent().unwrap();
        std::fs::create_dir_all(parent).expect("failed to create parent directory");

        let status = Command::new("git")
            .args(["clone", git])
            .arg(dep_dir.to_str().unwrap())
            .status()
            .expect("failed to run git clone");
        assert!(status.success(), "git clone failed");

        let status = Command::new("git")
            .args(["checkout", commit])
            .current_dir(dep_dir)
            .status()
            .expect("failed to run git checkout");
        assert!(status.success(), "git checkout failed");
    }

    fn dep_git_url<'a>(dep: &'a Dependency) -> &'a str {
        match &dep.dep {
            DependencyType::Git { git, .. } => git,
        }
    }

    fn dep_commit<'a>(dep: &'a Dependency) -> &'a str {
        match &dep.dep {
            DependencyType::Git { commit, .. } => commit,
        }
    }

    pub fn compile_private() {
        println!("cargo:rerun-if-changed=private.toml");

        let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let manifest_path = manifest_dir.join("private.toml");
        let content = std::fs::read_to_string(&manifest_path).expect("failed to read private.toml");
        let manifest: Manifest = toml::from_str(&content).expect("failed to parse private.toml");

        // Collect dependencies whose feature is enabled
        let mut deps_to_build: Vec<String> = Vec::new();
        for (feat, dep_names) in &manifest.features {
            if is_feature_enabled(feat) {
                for dep_name in dep_names {
                    if !deps_to_build.contains(dep_name) {
                        deps_to_build.push(dep_name.clone());
                    }
                }
            }
        }

        if deps_to_build.is_empty() {
            return;
        }

        let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
        let target = std::env::var("TARGET").unwrap();
        let is_debug = match std::env::var("PROFILE").as_deref() {
            Ok("debug") => true,
            Ok("release") => false,
            _ => std::env::var("DEBUG").unwrap_or_default() == "true",
        };

        for dep_name in &deps_to_build {
            let dep = manifest
                .dependencies
                .get(dep_name)
                .unwrap_or_else(|| panic!("dependency '{}' not found in private.toml", dep_name));

            let package_name = dep.package.as_ref().unwrap_or(dep_name);
            let dep_dir = out_dir.join(package_name);

            clone_and_checkout(&dep_dir, dep_git_url(dep), dep_commit(dep));

            let target_dir = dep_dir.join("target");
            let mut cmd = Command::new("cargo");
            cmd.args(["build", "-p", package_name])
                .arg("--target-dir")
                .arg(&target_dir)
                .arg("--target")
                .arg(&target);

            if !dep.features.is_empty() {
                cmd.arg("--features");
                cmd.arg(dep.features.join(","));
            }
            if !dep.default_features {
                cmd.arg("--no-default-features");
            }
            if !is_debug {
                cmd.arg("--release");
            }

            let status = cmd
                .current_dir(&dep_dir)
                .status()
                .expect("failed to run cargo build");
            assert!(status.success(), "cargo build failed for '{}'", dep_name);

            let profile = if is_debug { "debug" } else { "release" };
            let build_dir = target_dir.join(&target).join(profile);
            println!("cargo:rustc-link-search={}", build_dir.display());

            let lib_name = package_name.replace('-', "_");
            println!("cargo:rustc-link-lib=static={}", lib_name);
        }
    }
}

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
    #[cfg(feature = "private")]
    private::compile_private();
}
