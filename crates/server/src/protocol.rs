use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    /// App version sent by the client binary.
    pub client_version: String,
    /// Stable machine identifier from the local platform.
    pub machine_id: String,
    /// Operating system name.
    pub os: String,
    /// CPU architecture.
    pub arch: String,
    /// Whether the client was built in debug mode.
    pub debug: bool,
    /// Human-readable application name.
    pub app_name: String,
    /// Protocol version for compatibility checks.
    pub protocol_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbySummary {
    /// Stable lobby code used internally and for private invites.
    pub lobby_code: String,
    /// Host display name.
    pub host_name: String,
}

/// Every message a client sends to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    /// Introduce yourself to the server.
    Join {
        /// Display name of the player.
        name: String,
        /// Client metadata for diagnostics and compatibility.
        client: ClientInfo,
    },
    /// Request to be placed into the matchmaking queue.
    StartMatchmaking,
    /// Request to leave the matchmaking queue.
    LeaveMatchmaking,
    /// Subscribe to and fetch the current public lobby list.
    RequestLobbyList,
    /// Create a lobby. Public lobbies appear in the browser; private ones require a code.
    CreateLobby {
        /// Whether the lobby should be private.
        private_lobby: bool,
    },
    /// Leave a private lobby that was previously created.
    LeaveLobby,
    /// Join a lobby by code. Public lobbies use the same code internally.
    JoinLobby {
        /// The case-insensitive lobby code.
        lobby_code: String,
    },
    /// Make a move in an active game.
    MakeMove {
        /// The game this move belongs to.
        game_id: String,
        /// Source square (0–63).
        from: u8,
        /// Destination square (0–63).
        to: u8,
    },
    /// Forfeit the active game.
    Resign {
        /// The game to resign from.
        game_id: String,
    },
    /// Ask for a rematch after the game ended.
    RequestRematch {
        /// The finished game to base the rematch on.
        game_id: String,
    },
    /// Accept a pending rematch request from the opponent.
    AcceptRematch {
        /// The finished game id that the request is tied to.
        game_id: String,
    },
    /// Decline a pending rematch request from the opponent.
    DeclineRematch {
        /// The finished game id that the request is tied to.
        game_id: String,
    },
    /// Send a chat message to both players in a match.
    SendChat {
        /// The active or recently finished game id that the chat belongs to.
        game_id: String,
        /// Chat message body.
        message: String,
    },
    /// Gracefully disconnect from the websocket session.
    Disconnect {
        /// Optional human-readable reason.
        reason: Option<String>,
    },
}

/// Every message the server sends to a client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ServerMessage {
    /// Confirmation that the client was accepted.
    Joined {
        /// Unique id assigned to this player by the server.
        player_id: String,
    },
    /// Confirmation that matchmaking has started.
    MatchmakingStarted,
    /// Confirmation that the client left the matchmaking queue.
    MatchmakingLeft,
    /// Current list of public lobbies available to join.
    LobbyList {
        /// Public lobbies currently open.
        lobbies: Vec<LobbySummary>,
    },
    /// A push update for the public lobby browser after the lobby set changes.
    LobbyListUpdated {
        /// Public lobbies currently open.
        lobbies: Vec<LobbySummary>,
    },
    /// Confirmation that a lobby was created.
    LobbyCreated {
        /// The short lobby code other players can join.
        lobby_code: String,
        /// Whether the lobby is private.
        private_lobby: bool,
    },
    /// Confirmation that a lobby was left.
    LobbyLeft,
    /// The server found an opponent and created a game.
    MatchFound {
        /// Unique id for the newly created game.
        game_id: String,
        /// Display name of the opponent.
        opponent_name: String,
        /// Colour assigned to *this* client ("white" or "black").
        your_color: String,
    },
    /// The opponent made a move.
    OpponentMove {
        /// The game this move belongs to.
        game_id: String,
        /// Source square.
        from: u8,
        /// Destination square.
        to: u8,
    },
    /// The game has ended.
    GameOver {
        /// The game that ended.
        game_id: String,
        /// Why the game ended ("resign", "disconnect", "decline_rematch").
        reason: String,
        /// Whether this client won ("win"), lost ("loss"), or it was a draw
        /// ("draw" — currently unused but reserved).
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<String>,
    },
    /// The opponent is asking for a rematch.
    RematchRequested {
        /// The finished game the request belongs to.
        game_id: String,
    },
    /// The opponent accepted the rematch; a new game has started.
    RematchAccepted {
        /// The previous (finished) game id.
        old_game_id: String,
        /// The new game id.
        new_game_id: String,
        /// Colour assigned to *this* client ("white" or "black").
        your_color: String,
    },
    /// The opponent declined the rematch.
    RematchDeclined {
        /// The finished game id.
        game_id: String,
    },
    /// The opponent disconnected during the game; the match is over.
    OpponentDisconnected {
        /// The game that was abandoned.
        game_id: String,
    },
    /// A chat message in the current match.
    ChatMessage {
        /// The game this chat belongs to.
        game_id: String,
        /// Display name of the sender.
        sender_name: String,
        /// Chat message body.
        message: String,
    },
    /// A generic error in response to a malformed or illegal request.
    Error {
        /// Human-readable explanation of what went wrong.
        message: String,
    },
}
