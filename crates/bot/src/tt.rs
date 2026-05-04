use chess_engine::Board;
use dashmap::DashMap;

type ZobristKey = u64;

/// Compute Zobrist key for the board — uses the cached incremental value.
#[inline]
pub fn zobrist_key(board: &Board) -> ZobristKey {
    board.zobrist
}

#[derive(Debug, Clone)]
pub struct TTEntry {
    pub score: i32,
    pub depth: u32,
    pub flag: TTFlag,
    pub best_move: Option<chess_engine::Move>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TTFlag {
    Exact,
    LowerBound,
    UpperBound,
}

pub struct TranspositionTable {
    map: DashMap<ZobristKey, TTEntry>,
}

impl Default for TranspositionTable {
    fn default() -> Self {
        Self::new()
    }
}

impl TranspositionTable {
    pub fn new() -> Self {
        Self {
            map: DashMap::with_capacity(1_048_576),
        }
    }

    pub fn get(&self, key: ZobristKey) -> Option<TTEntry> {
        self.map.get(&key).map(|e| e.clone())
    }

    pub fn store(&self, key: ZobristKey, entry: TTEntry) {
        // Always replace — depth-preferred replacement is fine for this engine size
        self.map.insert(key, entry);
    }

    #[allow(dead_code)]
    pub fn clear(&self) {
        self.map.clear();
    }
}
