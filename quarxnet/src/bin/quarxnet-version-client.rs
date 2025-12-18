use std::io;
use std::net::TcpStream;

use chrono::Local;
use quarxtor_core::net_core::{NetRequest, NetResponse, VersionInfo};
use quarxnet::transport::simple::{recv_message, send_message};
use quarxnet::config::resolve_addr;

fn ts() -> String {
    let now = Local::now();
    now.format("[%Y-%m-%d %H:%M:%S%.3f]").to_string()
}

fn main() -> io::Result<()> {
    let addr = resolve_addr(std::env::args().nth(1));

    let mut stream = TcpStream::connect(&addr)?;
    eprintln!("{} [ver-cli] connected to {}", ts(), addr);

    let req = NetRequest::GetVersion;
    send_message(&mut stream, &req)?;
    eprintln!("{} [ver-cli] sent: {:?}", ts(), req);

    let resp: NetResponse = recv_message(&mut stream)?;
    match resp {
        NetResponse::Version(VersionInfo { name, version }) => {
            println!("{} [ver-cli] server: {name}, version: {version}", ts());
        }
        other => {
            println!("{} [ver-cli] unexpected response: {:?}", ts(), other);
        }
    }

    Ok(())
}
