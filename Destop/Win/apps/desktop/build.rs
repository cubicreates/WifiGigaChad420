use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=ui/bundle.mjs");
    println!("cargo:rerun-if-changed=ui/src/styles/app.css");
    println!("cargo:rerun-if-changed=ui/src/types/telemetry.ts");
    println!("cargo:rerun-if-changed=ui/src/components/Sidebar.tsx");
    println!("cargo:rerun-if-changed=ui/src/components/UpdateModal.tsx");
    println!("cargo:rerun-if-changed=ui/src/screens/InstallerUninstallerScreens.tsx");
    println!("cargo:rerun-if-changed=ui/src/screens/WifiDashboardScreen.tsx");
    println!("cargo:rerun-if-changed=ui/src/screens/OngoingProcessesScreen.tsx");
    println!("cargo:rerun-if-changed=ui/src/screens/SystemMeshInfoScreen.tsx");
    println!("cargo:rerun-if-changed=ui/src/App.tsx");
    println!("cargo:rerun-if-env-changed=GIGA_REQUIRE_ADMIN_MANIFEST");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let ui_dir = manifest_dir.join("ui");
    let bundler_script = ui_dir.join("bundle.mjs");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let out_html = out_dir.join("ui_bundle.html");

    let status = Command::new("node")
        .arg(&bundler_script)
        .arg(&ui_dir)
        .arg(&out_html)
        .status()
        .expect("Failed to execute node ui/bundle.mjs to compile .tsx files");

    assert!(status.success(), "ui/bundle.mjs failed to compile .tsx files");

    if env::var("GIGA_REQUIRE_ADMIN_MANIFEST").as_deref() == Ok("1") {
        #[cfg(windows)]
        {
            println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
            println!(
                "cargo:rustc-link-arg-bins=/MANIFESTUAC:level='requireAdministrator' uiAccess='false'"
            );
        }
    }
}
