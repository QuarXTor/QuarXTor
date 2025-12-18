use std::env;
use std::process;

use quarxnet::sync::client_hello;
use quarxnet::TcpTransport;
use quarxtor_core::net_core::{NodeId, ProtocolVersion};

fn usage() -> ! {
    eprintln!("Usage: quarxnet-hello [--host HOST] [--port PORT]");
    eprintln!("Env overrides: QUARXNET_HOST, QUARXNET_PORT");
    process::exit(1);
}

fn main() {
    // 1) ENV defaults
    let mut host = env::var("QUARXNET_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let mut port: u16 = env::var("QUARXNET_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(54721);

    // 2) CLI overrides
    let args: Vec<String> = env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--host" | "-h" => {
                i += 1;
                if i >= args.len() {
                    usage();
                }
                host = args[i].clone();
            }
            "--port" | "-p" => {
                i += 1;
                if i >= args.len() {
                    usage();
                }
                match args[i].parse::<u16>() {
                    Ok(p) => port = p,
                    Err(_) => {
                        eprintln!("[quarxnet-hello] invalid port: {}", args[i]);
                        process::exit(1);
                    }
                }
            }
            "--help" | "-?" => usage(),
            _ => usage(),
        }
        i += 1;
    }

    let addr = format!("{}:{}", host, port);
    eprintln!("[quarxnet-hello] connecting to {}", addr);

    let mut transport = match TcpTransport::connect(&addr) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[quarxnet-hello] connect error: {:?}", e);
            process::exit(1);
        }
    };

    let node: NodeId = 42;
    let version = ProtocolVersion { major: 0, minor: 1 };

    match client_hello(&mut transport, node, version) {
        Ok(resp) => {
            println!("[OK] Hello response: {:?}", resp);
        }
        Err(e) => {
            eprintln!("[quarxnet-hello] hello error: {:?}", e);
            process::exit(1);
        }
    }
}
