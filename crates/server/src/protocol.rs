use serde::{Deserialize, Serialize};

/// Every message a client sends to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    /// Introduce yourself to the server.
    Join {
        /// Display name of the player.
        name: String,
    },
    /// Request to be placed into the matchmaking queue.
    StartMatchmaking,
    /// Request to leave the matchmaking queue.
    LeaveMatchmaking,
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
