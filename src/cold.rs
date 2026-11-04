//! Compressed storage for the oldest scrollback rows (feature `compress`).
//!
//! Scrollback is the `cold` blocks (oldest first, zstd-compressed) followed by the hot `rows`
//! (plain, the newest). The viewport only ever reads hot rows: moving it deeper thaws blocks back
//! into `rows`, and rows far behind it are frozen again.

use alloc::{collections::VecDeque, vec::Vec};

/// Rows per frozen block.
const BLOCK: usize = 256;
/// Hot rows kept beyond the viewport, so scrolling a little never compresses or decompresses.
const MARGIN: usize = 512;
const LEVEL: i32 = 3;

#[derive(Clone, Debug)]
pub struct Block {
    data: Vec<u8>,
    /// Size of the data once decompressed.
    raw: usize,
    rows: usize,
    /// Stored uncompressed (compression failed).
    stored: bool,
    /// Whether any row carries a prompt mark.
    marked: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Cold {
    blocks: VecDeque<Block>,
    /// Rows of the oldest block already dropped at capacity.
    skip: usize,
}

impl Cold {
    /// Rows held (the oldest block's dropped ones excluded).
    pub fn len(&self) -> usize {
        self.blocks.iter().map(|b| b.rows).sum::<usize>() - self.skip
    }

    pub fn has_marks(&self) -> bool {
        self.blocks.iter().any(|b| b.marked)
    }

    pub fn clear(&mut self) {
        self.blocks.clear();
        self.skip = 0;
    }

    /// Drops the `n` oldest rows (at most `len()`).
    pub fn evict(&mut self, mut n: usize) {
        while n > 0 {
            let Some(front) = self.blocks.front() else { return };
            let left = front.rows - self.skip;
            if n >= left {
                self.blocks.pop_front();
                self.skip = 0;
                n -= left;
            } else {
                self.skip += n;
                n = 0;
            }
        }
    }

    /// Compresses `rows` as the newest block.
    pub fn freeze(&mut self, rows: impl Iterator<Item = crate::row::Row>) {
        let mut raw = Vec::new();
        let mut n = 0;
        let mut marked = false;
        for mut row in rows {
            // Rows the viewport widened to the screen's width go back to their text.
            row.trim();
            marked |= row.prompt_mark();
            row.encode(&mut raw);
            n += 1;
        }
        let (data, stored) = match zstd::bulk::compress(&raw, LEVEL) {
            Ok(data) => (data, false),
            Err(_) => (raw.clone(), true),
        };
        self.blocks.push_back(Block { data, raw: raw.len(), rows: n, stored, marked });
    }

    /// Takes the newest block back as plain rows (oldest first), if any.
    pub fn thaw(&mut self) -> Option<Vec<crate::row::Row>> {
        let block = self.blocks.pop_back()?;
        let skip = if self.blocks.is_empty() { core::mem::take(&mut self.skip) } else { 0 };
        let mut rows = decode(&block);
        rows.drain(..skip.min(rows.len()));
        Some(rows)
    }

    /// Every row, decompressed (oldest first), without changing anything.
    pub fn all_rows(&self) -> Vec<crate::row::Row> {
        let mut out: Vec<_> = self.blocks.iter().flat_map(decode).collect();
        out.drain(..self.skip.min(out.len()));
        out
    }
}

fn decode(block: &Block) -> Vec<crate::row::Row> {
    let raw = if block.stored {
        block.data.clone()
    } else {
        zstd::bulk::decompress(&block.data, block.raw).unwrap_or_default()
    };
    let mut pos = 0;
    let mut rows = Vec::with_capacity(block.rows);
    while pos < raw.len() {
        match crate::row::Row::decode(&raw, &mut pos) {
            Some(row) => rows.push(row),
            None => break,
        }
    }
    rows
}

/// Whether the hot rows (`hot`) hold enough beyond the viewport (`offset` rows back) to freeze a block.
pub fn can_freeze(hot: usize, offset: usize, screen_rows: usize) -> bool {
    hot >= offset + screen_rows + MARGIN + BLOCK
}

pub const FREEZE_ROWS: usize = BLOCK;
