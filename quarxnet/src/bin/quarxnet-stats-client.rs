use std::io;
use std::net::TcpStream;

use chrono::Local;
use quarxtor_core::net_core::{NetRequest, NetResponse, RepoStats};
use quarxnet::transport::simple::{recv_message, send_message};
use quarxnet::config::resolve_addr;

fn ts() -> String {
    let now = Local::now();
    now.format("[%Y-%m-%d %H:%M:%S%.3f]").to_string()
}

fn main() -> io::Result<()> {
    let addr = resolve_addr(std::env::args().nth(1));

    let mut stream = TcpStream::connect(&addr)?;
    eprintln!("{} [stats-cli] connected to {}", ts(), addr);

    let req = NetRequest::GetRepoStats;
    send_message(&mut stream, &req)?;
    eprintln!("{} [stats-cli] sent: {:?}", ts(), req);

    let resp: NetResponse = recv_message(&mut stream)?;
    match resp {
        NetResponse::RepoStats(RepoStats { blocks, bytes }) => {
            println!(
                "{} [stats-cli] repo stats: blocks={}, bytes={}",
                ts(),
                blocks,
                bytes
            );
        }
        other => {
            println!("{} [stats-cli] unexpected response: {:?}", ts(), other);
        }
    }

    Ok(())
}
