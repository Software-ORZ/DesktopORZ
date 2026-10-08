use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    // OUT_DIR = target/<perfil>/build/<pkg>-<hash>/out -> sobe 3 níveis até target/<perfil>
    let exe_dir = out_dir
        .ancestors()
        .nth(3)
        .expect("OUT_DIR inesperado")
        .to_path_buf();
    let langs_dir = exe_dir.join("langs");
    fs::create_dir_all(&langs_dir).expect("falha ao criar pasta langs");

    for entry in fs::read_dir("langs").expect("falha ao ler pasta langs") {
        let path = entry.expect("entrada inválida em langs").path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            println!("cargo:rerun-if-changed={}", path.display());
            let dest = langs_dir.join(path.file_name().unwrap());
            fs::copy(&path, &dest)
                .unwrap_or_else(|e| panic!("falha ao copiar {}: {e}", path.display()));
        }
    }

    embed_version_info();
}

/// Embute um recurso VERSIONINFO nos executáveis do pacote
/// (DesktopORZ.exe e DesktopORZ-HideIcons.exe). Metadados de versão
/// ajudam a reduzir falsos positivos de antivírus, que penalizam
/// binários sem CompanyName/FileDescription e de publisher desconhecido.
fn embed_version_info() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let mut parts = version.split('.').filter_map(|p| p.parse::<u64>().ok());
    let packed = (parts.next().unwrap_or(0) << 48)
        | (parts.next().unwrap_or(0) << 32)
        | (parts.next().unwrap_or(0) << 16)
        | parts.next().unwrap_or(0);

    let mut res = winresource::WindowsResource::new();
    // Manifesto com DPI-awareness PerMonitorV2: mantém as coordenadas sem
    // virtualização de DPI em setups multi-monitor. O recurso só é embutido
    // em alvos Windows.
    res.set_manifest(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="1.0.0.0" name="DesktopORZ.app" type="win32"/>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
    </windowsSettings>
  </application>
</assembly>"#,
    );
    res.set("CompanyName", "ThainanViniciusKatchan")
        .set("ProductName", &env::var("CARGO_PKG_NAME").unwrap())
        .set(
            "FileDescription",
            &env::var("CARGO_PKG_DESCRIPTION").unwrap(),
        )
        .set("InternalName", "DesktopORZ")
        .set("OriginalFilename", "DesktopORZ")
        .set(
            "LegalCopyright",
            "Copyright (C) 2026 ThainanViniciusKatchan",
        )
        .set_version_info(winresource::VersionInfo::FILEVERSION, packed)
        .set_version_info(winresource::VersionInfo::PRODUCTVERSION, packed);

    // Erros do compilador de recursos (rc.exe/windres ausente) não devem
    // derrubar o build inteiro: emite aviso e segue sem os metadados.
    if let Err(e) = res.compile() {
        println!("cargo:warning=falha ao embutir VERSIONINFO (metadados ignorados): {e}");
    }
}
