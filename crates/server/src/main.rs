use std::net::SocketAddr;

use chess_server::{GameServer, handle_connection};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = std::env::var("PORT").unwrap_or_else(|_| "20682".into());
    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .expect("invalid HOST address");

    let listener = TcpListener::bind(addr).await.expect("failed to bind");
    eprintln!("chess_server listening on ws://{addr}");

    let server = GameServer::new();

    loop {
        let (stream, peer) = listener.accept().await.unwrap();
        eprintln!("connection from {peer}");

        let sv = server.clone();
        tokio::spawn(async move {
            handle_connection(stream, sv).await;
            eprintln!("connection closed: {peer}");
        });
    }
}
