use std::fmt::Debug;

use serde::{Deserialize, Serialize};

/// Square grid, so this is the length of one row/column.
const BOARD_SIZE: usize = 3;

#[derive(Copy, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum Piece {
    #[default]
    Empty,
    Naught,
    Cross,
}

impl Debug for Piece {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Naught => write!(f, "O"),
            Self::Cross => write!(f, "X"),
            Self::Empty => write!(f, " "),
        }
    }
}

/// Basic board structure, 3x3 grid
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Board([Piece; BOARD_SIZE * BOARD_SIZE]);

impl Debug for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // print the board as a grid
        for row in self.0.iter().take(BOARD_SIZE) {
            write!(f, "{:?}", row)?;
            writeln!(f)?;
        }
        Ok(())
    }
}

impl Board {
    /// Create a new empty gameboard
    pub fn new() -> Self {
        Self([Piece::Empty; BOARD_SIZE * BOARD_SIZE])
    }
    /// Get the piece in a particular square
    pub fn get(&self, x: usize, y: usize) -> Piece {
        self.0[x + y * BOARD_SIZE]
    }
    /// Set the piece in a particular square
    pub fn set(&mut self, x: usize, y: usize, piece: Piece) {
        self.0[x + y * BOARD_SIZE] = piece;
    }
}
