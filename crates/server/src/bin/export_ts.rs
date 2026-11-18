//! Export the websocket protocol to TypeScript bindings.
//!
//! ```text
//! cargo run -p chess_server --features export-ts --bin export-ts -- <out-dir>
//! ```

use chess_server::protocol::{ClientInfo, ClientMessage, LobbySummary, ServerMessage};
use std::path::PathBuf;
use ts_rs::{Config, TS};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "bindings".to_string()),
    );

    let config = Config::new().with_out_dir(&out_dir);

    ClientMessage::export_all(&config)?;
    ServerMessage::export_all(&config)?;
    ClientInfo::export_all(&config)?;
    LobbySummary::export_all(&config)?;

    let file = out_dir.join("protocol.ts");
    println!("wrote {}", file.display());
    Ok(())
}
