// 构建脚本：把 .ico 与版本信息编进 exe 资源段
use std::path::{Path, PathBuf};
use std::process::Command;

fn find_rc() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("NV_RC") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(home) = std::env::var("USERPROFILE") {
        let base = PathBuf::from(home).join(".nuget/packages/microsoft.windows.sdk.buildtools");
        if let Ok(entries) = std::fs::read_dir(&base) {
            let mut versions: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            versions.sort();
            versions.reverse();
            for v in versions {
                if let Ok(bins) = std::fs::read_dir(v.join("bin")) {
                    for b in bins.flatten() {
                        let p = b.path().join("x64/rc.exe");
                        if p.exists() {
                            return Some(p);
                        }
                    }
                }
            }
        }
    }
    let p = PathBuf::from("rc.exe");
    if Command::new(&p).arg("/?").output().is_ok() {
        return Some(p);
    }
    None
}

fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");

    let assets = Path::new("assets");
    if !assets.join("icon.ico").exists() {
        println!("cargo:warning=assets/icon.ico 缺失，跳过图标资源");
        return;
    }
    let Some(rc) = find_rc() else {
        println!("cargo:warning=未找到 rc.exe，跳过图标资源");
        return;
    };
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default());
    let res = out_dir.join("app.res");
    let status = Command::new(&rc)
        .current_dir(assets)
        .arg("/nologo")
        .arg(format!("/fo{}", res.display()))
        .arg("app.rc")
        .output();
    match status {
        Ok(o) if o.status.success() => {
            println!("cargo:rustc-link-arg={}", res.display());
        }
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stdout).to_string();
            println!("cargo:warning=rc 失败: {}", err.replace('\n', " "));
        }
        _ => println!("cargo:warning=资源编译失败"),
    }
}
