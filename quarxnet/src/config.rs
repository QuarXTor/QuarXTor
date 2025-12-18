use std::path::PathBuf;

/// Canonical default address for QuarXTor net daemon.
const DEFAULT_ADDR: &str = "127.0.0.1:54721";

/// Address resolution:
///   1) if CLI argument is provided — use it as is;
///   2) otherwise try to read INI:
///      $XDG_CONFIG_HOME/quarxtor/quarxctl.ini
///      or $HOME/.config/quarxtor/quarxctl.ini
///      section [net], key addr = "HOST:PORT";
///   3) otherwise return DEFAULT_ADDR.
pub fn resolve_addr(cli_arg: Option<String>) -> String {
    if let Some(a) = cli_arg {
        return a;
    }

    if let Some(from_ini) = load_addr_from_ini() {
        return from_ini;
    }

    DEFAULT_ADDR.to_string()
}

fn load_addr_from_ini() -> Option<String> {
    let cfg_base: PathBuf = match std::env::var("XDG_CONFIG_HOME") {
        Ok(p) => PathBuf::from(p),
        Err(_) => {
            let home = std::env::var("HOME").ok()?;
            PathBuf::from(home).join(".config")
        }
    };

    let path = cfg_base.join("quarxtor").join("quarxctl.ini");
    if !path.exists() {
        return None;
    }

    // Minimal INI parser without external dependencies.
    let text = std::fs::read_to_string(path).ok()?;

    let mut in_net = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            let sect = &line[1..line.len() - 1].trim();
            in_net = sect.eq_ignore_ascii_case("net");
            continue;
        }

        if !in_net {
            continue;
        }

        // Look for addr=...
        if let Some(eq_pos) = line.find('=') {
            let (k, v) = line.split_at(eq_pos);
            let key = k.trim();
            if key.eq_ignore_ascii_case("addr") {
                let val = v[1..].trim(); // skip '='
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }

    None
}
