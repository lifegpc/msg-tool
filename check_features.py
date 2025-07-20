import toml
import subprocess
import sys

def main():
    # 检查cargo是否可用
    try:
        subprocess.run(
            ["cargo", "--version"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=True
        )
    except FileNotFoundError:
        print("Error: 'cargo' not found. Install Rust and ensure it's in PATH.")
        sys.exit(1)
    except subprocess.CalledProcessError as e:
        print(f"Error checking cargo: {e}")
        sys.exit(1)

    # 读取Cargo.toml
    try:
        with open("Cargo.toml", "r") as f:
            cargo_toml = toml.load(f)
    except FileNotFoundError:
        print("Error: Cargo.toml not found in current directory.")
        sys.exit(1)
    except Exception as e:
        print(f"Error parsing Cargo.toml: {e}")
        sys.exit(1)
    
    features = cargo_toml.get("features", {})
    feature_names = list(features.keys())
    feature_names = [name for name in feature_names if not name.startswith("utils-")]
    
    if not feature_names:
        print("No features defined in Cargo.toml.")
        sys.exit(0)
    
    failed_features = []
    test_failed_features = []
    
    print(f"Testing {len(feature_names)} features...")
    for idx, feature in enumerate(feature_names, 1):
        print(f"\nTesting feature {idx}/{len(feature_names)}: {feature}")
        cmd = ["cargo", "check", "--no-default-features", "--features", feature, '--target-dir', 'target/features_check']
        try:
            subprocess.run(cmd, check=True)
        except subprocess.CalledProcessError:
            failed_features.append(feature)
            print(f"❌ Feature '{feature}' failed to compile")
        else:
            print(f"✅ Feature '{feature}' compiled successfully")
        
        cmd = ["cargo", "test", "--no-default-features", "--features", feature, '--target-dir', 'target/features_check']
        try:
            subprocess.run(cmd, check=True)
        except subprocess.CalledProcessError:
            test_failed_features.append(feature)
            print(f"❌ Tests for feature '{feature}' failed")
        else:
            print(f"✅ Tests for feature '{feature}' passed")
    
    if failed_features or test_failed_features:
        if failed_features:
            print("\nFailed features:")
            for f in failed_features:
                print(f"  - {f}")
        if test_failed_features:
            print("\nFailed tests for features:")
            for f in test_failed_features:
                print(f"  - {f}")
        sys.exit(1)
    else:
        print("\nAll features compiled successfully!")
        sys.exit(0)

if __name__ == "__main__":
    main()
