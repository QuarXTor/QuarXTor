use std::io;
use std::net::{TcpListener, TcpStream};

use chrono::Local;
use quarxtor_core::net_core::{NetRequest, NetResponse, VersionInfo, RepoStats};
use quarxnet::transport::simple::{recv_message, send_message};

fn ts() -> String {
    let now = Local::now();
    now.format("[%Y-%m-%d %H:%M:%S%.3f]").to_string()
}

fn handle_client(mut stream: TcpStream) -> io::Result<()> {
    let req: NetRequest = recv_message(&mut stream)?;
    eprintln!("{} [srv] received: {:?}", ts(), req);

    let resp = match req {
        NetRequest::Ping => NetResponse::Pong,
        NetRequest::GetVersion => NetResponse::Version(VersionInfo {
            name: "quarxnet-ping-server".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }),
        NetRequest::GetRepoStats => {
            // Minimal stub: 0 blocks, 0 bytes.
            NetResponse::RepoStats(RepoStats {
                blocks: 0,
                bytes: 0,
            })
        }
    };

    send_message(&mut stream, &resp)?;
    Ok(())
}

fn main() -> io::Result<()> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:9345".to_string());

    let listener = TcpListener::bind(&addr)?;
    eprintln!("{} [srv] listening on {addr}", ts());

    for conn in listener.incoming() {
        match conn {
            Ok(stream) => {
                if let Err(e) = handle_client(stream) {
                    eprintln!("{} [srv] error handling client: {e}", ts());
                }
            }
            Err(e) => eprintln!("{} [srv] accept error: {e}", ts()),
        }
    }

    Ok(())
}
