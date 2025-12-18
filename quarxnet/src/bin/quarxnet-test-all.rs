use std::env;
use std::process;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use quarxnet::server::DEFAULT_PORT;
use quarxnet::{client_get_blocks, client_hello, client_push_blocks, TcpTransport};
use quarxtor_core::net_core::{NodeId, ProtocolVersion};

struct Row {
    name: &'static str,
    bytes: usize,
    ms: f64,
    status: &'static str,
}

fn usage() -> ! {
    eprintln!("Usage: quarxnet-test-all [--host HOST] [--port PORT]");
    eprintln!("Defaults: HOST=127.0.0.1, PORT={}", DEFAULT_PORT);
    process::exit(1);
}

fn checksum(data: &[u8]) -> u64 {
    data.iter().fold(0u64, |acc, &b| acc.wrapping_add(b as u64))
}

fn ts() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let ms = now.subsec_millis();
    let day = secs % 86_400;
    let h = day / 3600;
    let m = (day / 60) % 60;
    let s = day % 60;
    format!("[{:02}:{:02}:{:02}.{:03}]", h, m, s, ms)
}

fn main() {
    // defaults
    let mut host = "127.0.0.1".to_string();
    let mut port: u16 = DEFAULT_PORT;

    // parse CLI
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
                        eprintln!("{} [quarxnet-test-all] invalid port: {}", ts(), args[i]);
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
    eprintln!("{} [quarxnet-test-all] connecting to {addr}", ts());

    let mut transport = match TcpTransport::connect(&addr) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{} [quarxnet-test-all] connect error: {:?}", ts(), e);
            process::exit(1);
        }
    };

    let mut rows: Vec<Row> = Vec::new();
    let mut ok = true;

    // 1) Hello
    let node_id: NodeId = 42;
    let version = ProtocolVersion { major: 0, minor: 1 };

    let t0 = Instant::now();
    match client_hello(&mut transport, node_id, version) {
        Ok(resp) => {
            let dt = t0.elapsed();
            rows.push(Row {
                name: "hello",
                bytes: 0,
                ms: dt.as_secs_f64() * 1000.0,
                status: "OK",
            });
            eprintln!(
                "[OK] Hello response: node={}, version={}.{}",
                resp.node, resp.version.major, resp.version.minor
            );
        }
        Err(e) => {
            let dt = t0.elapsed();
            rows.push(Row {
                name: "hello",
                bytes: 0,
                ms: dt.as_secs_f64() * 1000.0,
                status: "ERR",
            });
            eprintln!("{} [ERR] hello failed: {:?}", ts(), e);
            ok = false;
        }
    }

    // 2) Generate test block 8K
    let test_len: usize = 8192;
    let mut test_buf = vec![0u8; test_len];
    for (i, b) in test_buf.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    let test_ck = checksum(&test_buf);

    // 3) PushBlocks
    let t1 = Instant::now();
    match client_push_blocks(&mut transport, &test_buf) {
        Ok(()) => {
            let dt = t1.elapsed();
            rows.push(Row {
                name: "push_blocks",
                bytes: test_len,
                ms: dt.as_secs_f64() * 1000.0,
                status: "OK",
            });
            eprintln!("{} [OK] push_blocks {} bytes", ts(), test_len);
        }
        Err(e) => {
            let dt = t1.elapsed();
            rows.push(Row {
                name: "push_blocks",
                bytes: test_len,
                ms: dt.as_secs_f64() * 1000.0,
                status: "ERR",
            });
            eprintln!("{} [ERR] push_blocks failed: {:?}", ts(), e);
            ok = false;
        }
    }

    // 4) GetBlocks (session echo)
    let t2 = Instant::now();
    let mut get_status = "OK";
    let recv_len: usize;

    match client_get_blocks(&mut transport, &[1u64]) {
        Ok(raw) => {
            let dt = t2.elapsed();
            recv_len = raw.len();
            let ck2 = checksum(&raw);

            if recv_len == test_len && ck2 == test_ck {
                eprintln!(
                    "[OK] get_blocks {} bytes, checksum match (0x{:016x})",
                    recv_len, ck2
                );
            } else {
                eprintln!(
                    "[MISMATCH] get_blocks len={}, ck=0x{:016x}, expected len={}, ck=0x{:016x}",
                    recv_len, ck2, test_len, test_ck
                );
                get_status = "MISMATCH";
                ok = false;
            }

            rows.push(Row {
                name: "get_blocks",
                bytes: recv_len,
                ms: dt.as_secs_f64() * 1000.0,
                status: get_status,
            });
        }
        Err(e) => {
            let dt = t2.elapsed();
            rows.push(Row {
                name: "get_blocks",
                bytes: 0,
                ms: dt.as_secs_f64() * 1000.0,
                status: "ERR",
            });
            eprintln!("{} [ERR] get_blocks failed: {:?}", ts(), e);
            ok = false;
        }
    }

    // 5) Results table
    println!();
    println!("┌───────────────┬────────────┬────────────┬──────────┐");
    println!("│ operation     │ bytes      │ time (ms)  │ status   │");
    println!("├───────────────┼────────────┼────────────┼──────────┤");
    for r in &rows {
        println!(
            "│ {:<13} │ {:>10} │ {:>10.3} │ {:<8} │",
            r.name, r.bytes, r.ms, r.status
        );
    }
    println!("└───────────────┴────────────┴────────────┴──────────┘");

    if !ok {
        process::exit(1);
    }
}
