use crate::protocol::{ClientInfo, ClientMessage, LobbySummary, ServerMessage};
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
type LobbyCode = String;

#[derive(Debug, Clone, PartialEq)]
enum PlayerState {
    Idle,
    BrowsingLobbies,
    InMatchmaking,
    InLobby(LobbyCode),
    InGame(GameId),
    GameOver(GameId),
}

struct Player {
    name: String,
    client_info: ClientInfo,
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

struct Lobby {
    host_id: PlayerId,
    private_lobby: bool,
}

// ---------------------------------------------------------------------------
// Shared server state
// ---------------------------------------------------------------------------

struct ServerInner {
    players: HashMap<PlayerId, Player>,
    matchmaking: Vec<PlayerId>,
    lobbies: HashMap<LobbyCode, Lobby>,
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
                lobbies: HashMap::new(),
                games: HashMap::new(),
            })),
        }
    }

    // ---- Player lifecycle ----

    /// Register a new player. Returns the assigned id.
    pub async fn register_player(
        &self,
        name: String,
        client_info: ClientInfo,
        tx: mpsc::UnboundedSender<ServerMessage>,
    ) -> PlayerId {
        let id = Uuid::new_v4().to_string();
        let mut inner = self.inner.write().await;
        inner.players.insert(
            id.clone(),
            Player {
                name,
                client_info,
                tx,
                state: PlayerState::Idle,
            },
        );
        if let Some(player) = inner.players.get(&id) {
            eprintln!(
                "Player connected: {} v{} {} {} hwid={}",
                player.name,
                player.client_info.client_version,
                player.client_info.os,
                player.client_info.arch,
                player.client_info.machine_id
            );
        }
        id
    }

    /// Remove a player entirely. Cleans up matchmaking queue and any active game.
    pub async fn remove_player(&self, player_id: &str) {
        let mut inner = self.inner.write().await;

        // Remove from matchmaking queue
        inner.matchmaking.retain(|p| p != player_id);

        // Remove hosted lobby if present
        let lobby_to_remove = inner.players.get(player_id).and_then(|player| match &player.state {
            PlayerState::InLobby(code) => Some(code.clone()),
            _ => None,
        });
        if let Some(code) = lobby_to_remove {
            inner.lobbies.remove(&code);
            self.broadcast_public_lobby_list_inner(&inner);
        }

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
        if let Some(player) = inner.players.remove(player_id) {
            eprintln!(
                "Player disconnected: {} v{} {} {} hwid={}",
                player.name,
                player.client_info.client_version,
                player.client_info.os,
                player.client_info.arch,
                player.client_info.machine_id
            );
        }

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
            .map(|p| matches!(p.state, PlayerState::Idle | PlayerState::BrowsingLobbies))
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

    pub async fn request_lobby_list(&self, player_id: &str) -> bool {
        let mut inner = self.inner.write().await;
        let can_browse = inner
            .players
            .get(player_id)
            .map(|p| matches!(p.state, PlayerState::Idle | PlayerState::BrowsingLobbies))
            .unwrap_or(false);
        if !can_browse {
            return false;
        }
        if let Some(player) = inner.players.get_mut(player_id) {
            player.state = PlayerState::BrowsingLobbies;
        }
        self.send_public_lobby_list_to_player_inner(&inner, player_id);
        true
    }

    pub async fn create_lobby(
        &self,
        player_id: &str,
        private_lobby: bool,
    ) -> Option<LobbyCode> {
        let mut inner = self.inner.write().await;

        let can_create = inner
            .players
            .get(player_id)
            .map(|p| matches!(p.state, PlayerState::Idle | PlayerState::BrowsingLobbies))
            .unwrap_or(false);
        if !can_create {
            return None;
        }

        let code = loop {
            let candidate = Uuid::new_v4()
                .simple()
                .to_string()
                .chars()
                .take(6)
                .collect::<String>()
                .to_uppercase();
            if !inner.lobbies.contains_key(&candidate) {
                break candidate;
            }
        };

        inner.lobbies.insert(
            code.clone(),
            Lobby {
                host_id: player_id.to_string(),
                private_lobby,
            },
        );

        if let Some(player) = inner.players.get_mut(player_id) {
            player.state = PlayerState::InLobby(code.clone());
        }

        if !private_lobby {
            self.broadcast_public_lobby_list_inner(&inner);
        }

        Some(code)
    }

    pub async fn leave_lobby(&self, player_id: &str) -> bool {
        let mut inner = self.inner.write().await;

        let lobby_code = match inner.players.get(player_id).map(|p| p.state.clone()) {
            Some(PlayerState::InLobby(code)) => code,
            _ => return false,
        };

        let was_public = inner
            .lobbies
            .get(&lobby_code)
            .map(|lobby| !lobby.private_lobby)
            .unwrap_or(false);
        inner.lobbies.remove(&lobby_code);
        if let Some(player) = inner.players.get_mut(player_id) {
            player.state = PlayerState::Idle;
        }
        if was_public {
            self.broadcast_public_lobby_list_inner(&inner);
        }
        true
    }

    pub async fn join_lobby(&self, player_id: &str, lobby_code: &str) -> Result<(), String> {
        let mut inner = self.inner.write().await;

        let can_join = inner
            .players
            .get(player_id)
            .map(|p| matches!(p.state, PlayerState::Idle | PlayerState::BrowsingLobbies))
            .unwrap_or(false);
        if !can_join {
            return Err("you are not available to join a lobby".into());
        }

        let normalized_code = lobby_code.trim().to_uppercase();
        if normalized_code.is_empty() {
            return Err("lobby code is required".into());
        }

        let (host_id, was_public) = match inner.lobbies.get(&normalized_code) {
            Some(lobby) => (lobby.host_id.clone(), !lobby.private_lobby),
            None => return Err("lobby not found".into()),
        };

        if host_id == player_id {
            return Err("you cannot join your own lobby".into());
        }

        let host_ready = inner
            .players
            .get(&host_id)
            .map(|p| p.state == PlayerState::InLobby(normalized_code.clone()))
            .unwrap_or(false);
        if !host_ready {
            inner.lobbies.remove(&normalized_code);
            if was_public {
                self.broadcast_public_lobby_list_inner(&inner);
            }
            return Err("lobby is no longer available".into());
        }

        inner.lobbies.remove(&normalized_code);
        if was_public {
            self.broadcast_public_lobby_list_inner(&inner);
        }
        self.create_game_inner(&mut inner, &host_id, player_id);
        Ok(())
    }

    fn public_lobby_summaries_inner(&self, inner: &ServerInner) -> Vec<LobbySummary> {
        inner
            .lobbies
            .iter()
            .filter_map(|(code, lobby)| {
                if lobby.private_lobby {
                    return None;
                }
                let host_name = inner.players.get(&lobby.host_id)?.name.clone();
                Some(LobbySummary {
                    lobby_code: code.clone(),
                    host_name,
                })
            })
            .collect()
    }

    fn send_public_lobby_list_to_player_inner(&self, inner: &ServerInner, player_id: &str) {
        let lobbies = self.public_lobby_summaries_inner(inner);
        if let Some(player) = inner.players.get(player_id) {
            let _ = player.tx.send(ServerMessage::LobbyList { lobbies });
        }
    }

    fn broadcast_public_lobby_list_inner(&self, inner: &ServerInner) {
        let lobbies = self.public_lobby_summaries_inner(inner);
        for player in inner.players.values() {
            if matches!(player.state, PlayerState::BrowsingLobbies) {
                let _ = player.tx.send(ServerMessage::LobbyListUpdated {
                    lobbies: lobbies.clone(),
                });
            }
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
            ClientMessage::Join { name, client } => {
                if player_id.is_some() {
                    let _ = tx.send(ServerMessage::Error {
                        message: "already joined".into(),
                    });
                    continue;
                }
                let id = server.register_player(name, client, tx.clone()).await;
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

            ClientMessage::RequestLobbyList => {
                if let Some(pid) = &player_id
                    && !server.request_lobby_list(pid).await
                {
                    let _ = tx.send(ServerMessage::Error {
                        message: "unable to browse lobbies right now".into(),
                    });
                }
            }

            ClientMessage::CreateLobby { private_lobby } => {
                if let Some(pid) = &player_id {
                    match server.create_lobby(pid, private_lobby).await {
                        Some(lobby_code) => {
                            let _ = tx.send(ServerMessage::LobbyCreated {
                                lobby_code,
                                private_lobby,
                            });
                        }
                        None => {
                            let _ = tx.send(ServerMessage::Error {
                                message: "unable to create lobby right now".into(),
                            });
                        }
                    }
                }
            }

            ClientMessage::LeaveLobby => {
                if let Some(pid) = &player_id
                    && server.leave_lobby(pid).await
                {
                    let _ = tx.send(ServerMessage::LobbyLeft);
                }
            }

            ClientMessage::JoinLobby { lobby_code } => {
                if let Some(pid) = &player_id
                    && let Err(message) = server.join_lobby(pid, &lobby_code).await
                {
                    let _ = tx.send(ServerMessage::Error { message });
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
            ClientMessage::Disconnect { reason } => {
                if let Some(pid) = &player_id {
                    eprintln!(
                        "Graceful disconnect from {}: {}",
                        pid,
                        reason.unwrap_or_else(|| "no reason provided".into())
                    );
                }
                break;
            }
        }
    }

    // Client disconnected — clean up
    if let Some(pid) = &player_id {
        server.remove_player(pid).await;
    }

    write_task.await.unwrap_or(());
}
