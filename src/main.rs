mod forward;

fn main() {
    std::fs::remove_dir_all("E:/code/ftcp/test").unwrap_or_default();
    std::fs::create_dir_all("E:/code/ftcp/test").unwrap();
    forward::TcpProxyClient::new(59238, "E:/code/ftcp/test".to_owned(), true)
        .expect("Fail to create the client");
    loop {}
}
