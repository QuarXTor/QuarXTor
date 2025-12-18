use std::io;
use std::net::TcpListener;
use std::os::unix::net::UnixListener;
use std::path::Path;
use chrono::Local;
use quarxtor_core::config::EffectiveConfig;
use quarxnet::addr::parse_bind_addr;
use quarxnet::proto0::serve_one_proto0;

fn ts() -> String {
    let now = Local::now();
    now.format("[%Y-%m-%d %H:%M:%S%.3f]").to_string()
}

/// Minimal proto0 handler: PING => PONG.
fn handle_proto0<S: io::Read + io::Write>(mut stream: S) -> io::Result<()> {
    serve_one_proto0(&mut stream).map_err(|e| {
        eprintln!("{} [quarxnetd] proto error: {}", ts(), e);
        io::Error::new(io::ErrorKind::Other, e)
    })
}

fn main() -> io::Result<()> {
    let mut args = std::env::args().skip(1);
    let dry_run = args.any(|a| a == "--dry-run");
    let cfg = EffectiveConfig::load_strict().expect("invalid config");
    let addr = cfg.net_addr.clone();
    let bind_addr = parse_bind_addr(&addr).expect("invalid bind address");

    eprintln!(
        "{} [quarxnetd] starting… version={}, bind={}",
        ts(),
        env!("CARGO_PKG_VERSION"),
        addr
    );

    if dry_run {
        eprintln!("{} [quarxnetd] READY (dry-run) bind={}", ts(), bind_addr);
        return Ok(());
    }

    if cfg.net_transport == "unix" {
        let path = Path::new(&cfg.net_unix_path);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path).map_err(|e| {
            eprintln!("{} [quarxnetd] bind error on {}: {}", ts(), cfg.net_unix_path, e);
            e
        })?;
        eprintln!("{} [quarxnetd] listening unix {}", ts(), cfg.net_unix_path);
        for conn in listener.incoming() {
            match conn {
                Ok(stream) => {
                    if let Err(e) = handle_proto0(stream) {
                        eprintln!("{} [quarxnetd] client error: {}", ts(), e);
                    }
                }
                Err(e) => {
                    eprintln!("{} [quarxnetd] accept error: {}", ts(), e);
                }
            }
        }
        let _ = std::fs::remove_file(path);
        Ok(())
    } else {
        let listener = TcpListener::bind(&bind_addr).map_err(|e| {
            eprintln!(
                "{} [quarxnetd] bind error on {}: {}",
                ts(),
                bind_addr,
                e
            );
            e
        })?;
        eprintln!("{} [quarxnetd] listening on {}", ts(), addr);

        for conn in listener.incoming() {
            match conn {
                Ok(stream) => {
                    if let Err(e) = handle_proto0(stream) {
                        eprintln!("{} [quarxnetd] client error: {}", ts(), e);
                    }
                }
                Err(e) => {
                    eprintln!("{} [quarxnetd] accept error: {}", ts(), e);
                }
            }
        }
        Ok(())
    }
}
