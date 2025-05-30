use board::Board;
use iroh::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub mod board;

#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayerType {
    #[default]
    Spectator,
    Human,
    Ai,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum Player {
    /// Local Node is responsible for this player
    Local(PlayerType),
    /// Remote Node is responsible for this player
    Remote(NodeId),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum GameState {
    /// Lobby waiting to assign players ready to start game.
    New,
    /// Game is in progress
    InProgress { turn: usize, board: Board },
    /// Game is over
    Finished { winner: Player },
}

/// Game information
#[derive(Debug, Serialize, Deserialize)]
pub struct Game {
    /// Game state
    pub state: GameState,
    /// Participating nodes, including spectators
    pub participants: HashSet<Player>,
}

impl Game {
    /// Create a new game
    pub fn new() -> Self {
        Self {
            state: GameState::New,
            participants: HashSet::new(),
        }
    }
    pub fn start_game(&mut self) {
        self.state = GameState::InProgress {
            turn: 0,
            board: Board::new(),
        }
    }
}
