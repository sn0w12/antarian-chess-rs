use crate::protocol::{ClientMessage, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{RwLock, mpsc};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

type PlayerId = String;
type GameId = String;

#[derive(Debug, Clone, PartialEq)]
enum PlayerState {
    Idle,
    InMatchmaking,
    InGame(GameId),
    GameOver(GameId),
}

struct Player {
    name: String,
    tx: mpsc::UnboundedSender<ServerMessage>,
    state: PlayerState,
}

struct Game {
    id: GameId,
    white_id: PlayerId,
    black_id: PlayerId,
    over: bool,
    /// Player id who has asked for a rematch (if any).
    rematch_requester: Option<PlayerId>,
}

// ---------------------------------------------------------------------------
// Shared server state
// ---------------------------------------------------------------------------

struct ServerInner {
    players: HashMap<PlayerId, Player>,
    matchmaking: Vec<PlayerId>,
    games: HashMap<GameId, Game>,
}

#[derive(Clone)]
pub struct GameServer {
    inner: Arc<RwLock<ServerInner>>,
}

impl Default for GameServer {
    fn default() -> Self {
        Self::new()
    }
}

impl GameServer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(ServerInner {
                players: HashMap::new(),
                matchmaking: Vec::new(),
                games: HashMap::new(),
            })),
        }
    }

    // ---- Player lifecycle ----

    /// Register a new player. Returns the assigned id.
    pub async fn register_player(
        &self,
        name: String,
        tx: mpsc::UnboundedSender<ServerMessage>,
    ) -> PlayerId {
        let id = Uuid::new_v4().to_string();
        let mut inner = self.inner.write().await;
        inner.players.insert(
            id.clone(),
            Player {
                name,
                tx,
                state: PlayerState::Idle,
            },
        );
        id
    }

    /// Remove a player entirely. Cleans up matchmaking queue and any active game.
    pub async fn remove_player(&self, player_id: &str) {
        let mut inner = self.inner.write().await;

        // Remove from matchmaking queue
        inner.matchmaking.retain(|p| p != player_id);

        // Gather info about what game the player was in before the player is removed
        let game_info: Vec<(GameId, PlayerId, bool)> = {
            if let Some(player) = inner.players.get(player_id) {
                match &player.state {
                    PlayerState::InGame(gid) | PlayerState::GameOver(gid) => {
                        if let Some(game) = inner.games.get(gid) {
                            let opponent_id = if game.white_id == player_id {
                                game.black_id.clone()
                            } else {
                                game.white_id.clone()
                            };
                            vec![(gid.clone(), opponent_id, game.over)]
                        } else {
                            vec![]
                        }
                    }
                    _ => vec![],
                }
            } else {
                vec![]
            }
        };

        // Remove the player
        inner.players.remove(player_id);

        // Notify opponent and clean up games
        for (gid, opponent_id, was_over) in game_info {
            if was_over {
                if let Some(opponent) = inner.players.get(&opponent_id) {
                    let _ = opponent.tx.send(ServerMessage::GameOver {
                        game_id: gid.clone(),
                        reason: "opponent_left".into(),
                        result: None,
                    });
                }
                // Clean up the game
                inner.games.remove(&gid);
                if let Some(opponent) = inner.players.get_mut(&opponent_id) {
                    opponent.state = PlayerState::Idle;
                }
            } else {
                if let Some(opponent) = inner.players.get(&opponent_id) {
                    let _ = opponent.tx.send(ServerMessage::OpponentDisconnected {
                        game_id: gid.clone(),
                    });
                }
                // Move opponent to idle
                inner.games.remove(&gid);
                if let Some(opponent) = inner.players.get_mut(&opponent_id) {
                    opponent.state = PlayerState::Idle;
                }
            }
        }
    }

    // ---- Matchmaking ----

    /// Add a player to the matchmaking queue. Returns `true` when two players
    /// were paired immediately (the server already sent `MatchFound` to both).
    pub async fn start_matchmaking(&self, player_id: &str) -> bool {
        let mut inner = self.inner.write().await;

        let can_queue = inner
            .players
            .get(player_id)
            .map(|p| p.state == PlayerState::Idle)
            .unwrap_or(false);

        if !can_queue {
            return false;
        }

        if let Some(p) = inner.players.get_mut(player_id) {
            p.state = PlayerState::InMatchmaking;
        }
        inner.matchmaking.push(player_id.to_string());

        if inner.matchmaking.len() >= 2 {
            let p2 = inner.matchmaking.remove(0);
            let p1 = inner.matchmaking.remove(0);
            self.create_game_inner(&mut inner, &p1, &p2);
            true
        } else {
            if let Some(p) = inner.players.get(player_id) {
                let _ = p.tx.send(ServerMessage::MatchmakingStarted);
            }
            false
        }
    }

    pub async fn leave_matchmaking(&self, player_id: &str) {
        let mut inner = self.inner.write().await;
        inner.matchmaking.retain(|p| p != player_id);
        if let Some(p) = inner.players.get_mut(player_id)
            && p.state == PlayerState::InMatchmaking
        {
            p.state = PlayerState::Idle;
        }
    }

    fn create_game_inner(&self, inner: &mut ServerInner, p1_id: &str, p2_id: &str) {
        let game_id = Uuid::new_v4().to_string();

        let p1_name = inner
            .players
            .get(p1_id)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let p2_name = inner
            .players
            .get(p2_id)
            .map(|p| p.name.clone())
            .unwrap_or_default();

        inner.games.insert(
            game_id.clone(),
            Game {
                id: game_id.clone(),
                white_id: p1_id.to_string(),
                black_id: p2_id.to_string(),
                over: false,
                rematch_requester: None,
            },
        );

        if let Some(p) = inner.players.get_mut(p1_id) {
            p.state = PlayerState::InGame(game_id.clone());
            let _ = p.tx.send(ServerMessage::MatchFound {
                game_id: game_id.clone(),
                opponent_name: p2_name,
                your_color: "white".into(),
            });
        }
        if let Some(p) = inner.players.get_mut(p2_id) {
            p.state = PlayerState::InGame(game_id.clone());
            let _ = p.tx.send(ServerMessage::MatchFound {
                game_id,
                opponent_name: p1_name,
                your_color: "black".into(),
            });
        }
    }

    // ---- Game actions ----

    /// Forward a move from one player to the opponent in the specified game.
    pub async fn handle_move(&self, player_id: &str, game_id: &str, from: u8, to: u8) {
        let inner = self.inner.read().await;

        let game = match inner.games.get(game_id) {
            Some(g) => g,
            None => return,
        };
        if game.over {
            return;
        }

        let opponent_id = if game.white_id == player_id {
            &game.black_id
        } else if game.black_id == player_id {
            &game.white_id
        } else {
            return;
        };

        if let Some(opponent) = inner.players.get(opponent_id) {
            let _ = opponent.tx.send(ServerMessage::OpponentMove {
                game_id: game_id.to_string(),
                from,
                to,
            });
        }
    }

    /// Broadcast a validated chat message to both players in the specified game.
    pub async fn send_chat(&self, player_id: &str, game_id: &str, message: &str) {
        let trimmed = message.trim();
        if trimmed.is_empty() || trimmed.len() > 280 {
            return;
        }

        let inner = self.inner.read().await;

        let game = match inner.games.get(game_id) {
            Some(g) => g,
            None => return,
        };

        let (white_id, black_id, sender_name) = if game.white_id == player_id {
            let sender_name = inner
                .players
                .get(&game.white_id)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            (game.white_id.clone(), game.black_id.clone(), sender_name)
        } else if game.black_id == player_id {
            let sender_name = inner
                .players
                .get(&game.black_id)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            (game.white_id.clone(), game.black_id.clone(), sender_name)
        } else {
            return;
        };

        let payload = ServerMessage::ChatMessage {
            game_id: game_id.to_string(),
            sender_name,
            message: trimmed.to_string(),
        };

        if let Some(player) = inner.players.get(&white_id) {
            let _ = player.tx.send(payload.clone());
        }
        if let Some(player) = inner.players.get(&black_id) {
            let _ = player.tx.send(payload);
        }
    }

    /// Mark a game as resigned; the opponent wins.
    pub async fn resign(&self, player_id: &str, game_id: &str) {
        let mut inner = self.inner.write().await;

        let gid = game_id.to_string();
        let (winner_id, loser_id) = {
            let game = match inner.games.get_mut(game_id) {
                Some(g) if !g.over => g,
                _ => return,
            };
            game.over = true;

            if game.white_id == player_id {
                (game.black_id.clone(), game.white_id.clone())
            } else if game.black_id == player_id {
                (game.white_id.clone(), game.black_id.clone())
            } else {
                return;
            }
        };

        if let Some(p) = inner.players.get_mut(&winner_id) {
            p.state = PlayerState::GameOver(gid.clone());
            let _ = p.tx.send(ServerMessage::GameOver {
                game_id: gid.clone(),
                reason: "resign".into(),
                result: Some("win".into()),
            });
        }
        if let Some(p) = inner.players.get_mut(&loser_id) {
            p.state = PlayerState::GameOver(gid.clone());
            let _ = p.tx.send(ServerMessage::GameOver {
                game_id: gid,
                reason: "resign".into(),
                result: Some("loss".into()),
            });
        }
    }

    // ---- Rematch ----

    pub async fn request_rematch(&self, player_id: &str, game_id: &str) {
        let mut inner = self.inner.write().await;

        let opponent_id = {
            let game = match inner.games.get_mut(game_id) {
                Some(g) if g.over => g,
                _ => return,
            };
            if game.rematch_requester.is_some() {
                return;
            }
            game.rematch_requester = Some(player_id.to_string());
            if game.white_id == player_id {
                game.black_id.clone()
            } else {
                game.white_id.clone()
            }
        };

        if let Some(p) = inner.players.get(&opponent_id) {
            let _ = p.tx.send(ServerMessage::RematchRequested {
                game_id: game_id.to_string(),
            });
        }
    }

    pub async fn accept_rematch(&self, player_id: &str, game_id: &str) {
        let mut inner = self.inner.write().await;

        let (p1_id, p2_id, old_gid) = {
            let game = match inner.games.get(game_id) {
                Some(g) if g.over => g,
                _ => return,
            };
            let requester_id = match &game.rematch_requester {
                Some(r) => r.clone(),
                None => return,
            };
            if player_id != game.white_id && player_id != game.black_id {
                return;
            }
            if player_id == requester_id {
                return;
            }

            // Swap colours: the requester gets the opposite colour for the rematch
            if game.white_id == requester_id {
                (
                    game.white_id.clone(),
                    game.black_id.clone(),
                    game.id.clone(),
                )
            } else {
                (
                    game.black_id.clone(),
                    game.white_id.clone(),
                    game.id.clone(),
                )
            }
        };

        let new_game_id = Uuid::new_v4().to_string();

        inner.games.insert(
            new_game_id.clone(),
            Game {
                id: new_game_id.clone(),
                white_id: p1_id.clone(),
                black_id: p2_id.clone(),
                over: false,
                rematch_requester: None,
            },
        );

        if let Some(p) = inner.players.get_mut(&p1_id) {
            p.state = PlayerState::InGame(new_game_id.clone());
            let _ = p.tx.send(ServerMessage::RematchAccepted {
                old_game_id: old_gid.clone(),
                new_game_id: new_game_id.clone(),
                your_color: "white".into(),
            });
        }
        if let Some(p) = inner.players.get_mut(&p2_id) {
            p.state = PlayerState::InGame(new_game_id.clone());
            let _ = p.tx.send(ServerMessage::RematchAccepted {
                old_game_id: old_gid.clone(),
                new_game_id,
                your_color: "black".into(),
            });
        }

        // Remove the old game entry
        inner.games.remove(&old_gid);
    }

    pub async fn decline_rematch(&self, player_id: &str, game_id: &str) {
        let mut inner = self.inner.write().await;

        let requester_id = {
            let game = match inner.games.get(game_id) {
                Some(g) if g.over => g,
                _ => return,
            };
            let requester_id = match &game.rematch_requester {
                Some(r) => r.clone(),
                None => return,
            };
            if player_id != game.white_id && player_id != game.black_id {
                return;
            }
            if player_id == requester_id {
                return;
            }
            requester_id
        };

        // Tell the requester their rematch was declined
        if let Some(p) = inner.players.get(&requester_id) {
            let _ = p.tx.send(ServerMessage::RematchDeclined {
                game_id: game_id.to_string(),
            });
        }

        // Move both players back to idle
        let pids: Vec<PlayerId> = {
            if let Some(game) = inner.games.get(game_id) {
                vec![game.white_id.clone(), game.black_id.clone()]
            } else {
                vec![]
            }
        };
        for pid in &pids {
            if let Some(p) = inner.players.get_mut(pid) {
                p.state = PlayerState::Idle;
            }
        }

        // Also let the decliner know the rematch is off
        if let Some(p) = inner.players.get(player_id) {
            let _ = p.tx.send(ServerMessage::RematchDeclined {
                game_id: game_id.to_string(),
            });
        }

        inner.games.remove(game_id);
    }
}

// ---------------------------------------------------------------------------
// Per-connection handler
// ---------------------------------------------------------------------------

pub async fn handle_connection(stream: TcpStream, server: GameServer) {
    let ws = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("WebSocket handshake failed: {e}");
            return;
        }
    };

    let (mut ws_write, mut ws_read) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

    let mut player_id: Option<PlayerId> = None;

    // Spawn a task that forwards ServerMessages from the channel to the WS
    let write_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let payload = serde_json::to_string(&msg).unwrap_or_default();
            if ws_write.send(Message::Text(payload)).await.is_err() {
                break;
            }
        }
        let _ = ws_write.close().await;
    });

    // Read loop
    loop {
        let msg = match ws_read.next().await {
            Some(Ok(Message::Text(t))) => t,
            Some(Ok(Message::Close(_))) | None => break,
            Some(Err(e)) => {
                eprintln!("WS read error: {e}");
                break;
            }
            _ => continue,
        };

        let parsed: ClientMessage = match serde_json::from_str(&msg) {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(ServerMessage::Error {
                    message: format!("invalid message: {e}"),
                });
                continue;
            }
        };

        // Every first message from a connection MUST be Join
        if player_id.is_none() {
            match &parsed {
                ClientMessage::Join { .. } => {}
                _ => {
                    let _ = tx.send(ServerMessage::Error {
                        message: "first message must be Join".into(),
                    });
                    continue;
                }
            }
        }

        match parsed {
            ClientMessage::Join { name } => {
                if player_id.is_some() {
                    let _ = tx.send(ServerMessage::Error {
                        message: "already joined".into(),
                    });
                    continue;
                }
                let id = server.register_player(name, tx.clone()).await;
                let _ = tx.send(ServerMessage::Joined {
                    player_id: id.clone(),
                });
                player_id = Some(id);
            }

            ClientMessage::StartMatchmaking => {
                let pid = match &player_id {
                    Some(p) => p.clone(),
                    None => continue,
                };
                let _matched = server.start_matchmaking(&pid).await;
            }

            ClientMessage::LeaveMatchmaking => {
                if let Some(pid) = &player_id {
                    server.leave_matchmaking(pid).await;
                    let _ = tx.send(ServerMessage::MatchmakingLeft);
                }
            }

            ClientMessage::MakeMove { game_id, from, to } => {
                if let Some(pid) = &player_id {
                    server.handle_move(pid, &game_id, from, to).await;
                }
            }

            ClientMessage::Resign { game_id } => {
                if let Some(pid) = &player_id {
                    server.resign(pid, &game_id).await;
                }
            }

            ClientMessage::RequestRematch { game_id } => {
                if let Some(pid) = &player_id {
                    server.request_rematch(pid, &game_id).await;
                }
            }

            ClientMessage::AcceptRematch { game_id } => {
                if let Some(pid) = &player_id {
                    server.accept_rematch(pid, &game_id).await;
                }
            }

            ClientMessage::DeclineRematch { game_id } => {
                if let Some(pid) = &player_id {
                    server.decline_rematch(pid, &game_id).await;
                }
            }
            ClientMessage::SendChat { game_id, message } => {
                if let Some(pid) = &player_id {
                    server.send_chat(pid, &game_id, &message).await;
                }
            }
        }
    }

    // Client disconnected — clean up
    if let Some(pid) = &player_id {
        server.remove_player(pid).await;
    }

    write_task.await.unwrap_or(());
}
