use crate::addr::parse_bind_addr;

#[test]
fn parse_bind_addr_ok() {
    let addr = parse_bind_addr("127.0.0.1:39090").unwrap();
    assert_eq!(addr.port(), 39090);
    assert!(addr.ip().is_ipv4());
}

#[test]
fn parse_bind_addr_err() {
    let err = parse_bind_addr("bad-addr").unwrap_err();
    assert!(err.contains("net.addr"));
}
