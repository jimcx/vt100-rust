//! Benchmark: `cargo run --release --features compress --example bench_compress -- FILE ROWS COLS CAP`
//! Feeds the last lines of FILE (as `cat` would, CRLF) through a parser with and without
//! scrollback compression; prints live heap bytes (counting allocator) and search-sweep timings.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::Instant;

struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
fn bump(n: usize) {
    let now = LIVE.fetch_add(n, Relaxed) + n;
    PEAK.fetch_max(now, Relaxed);
}
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        bump(l.size());
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size(), Relaxed);
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        LIVE.fetch_sub(l.size(), Relaxed);
        bump(n);
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static A: Counting = Counting;

fn nz(n: u16) -> core::num::NonZero<u16> {
    core::num::NonZero::new(n).unwrap()
}

/// Walks the whole scrollback a page at a time, reading every cell, and counts `needle` hits per row.
fn search(p: &mut vt100::Parser, needle: &str) -> usize {
    let (rows, cols) = p.screen().size();
    let (rows, cols) = (rows.get(), cols.get());
    let max = p.screen().scrollback_len();
    let mut hits = 0;
    let mut off = max;
    loop {
        p.screen_mut().set_scrollback(off);
        for r in 0..rows {
            let mut line = String::new();
            for c in 0..cols {
                match p.screen().cell(r, c) {
                    Some(cell) if !cell.is_wide_continuation() => {
                        line.push_str(if cell.contents().is_empty() { " " } else { cell.contents() })
                    }
                    _ => {}
                }
            }
            hits += line.matches(needle).count();
        }
        if off == 0 {
            break;
        }
        off = off.saturating_sub(rows as usize);
    }
    p.screen_mut().set_scrollback(0);
    hits
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let text = std::fs::read(&a[1]).unwrap();
    let (rows, cols, cap): (u16, u16, usize) = (a[2].parse().unwrap(), a[3].parse().unwrap(), a[4].parse().unwrap());
    let lines: Vec<&[u8]> = text.split(|&b| b == b'\n').collect();
    let take = lines.len().min(cap + 2000);
    let lines = &lines[lines.len() - take..];
    println!("{}: {} lines fed, screen {rows}x{cols}, scrollback cap {cap}", a[1], lines.len());
    let mut base = 0;
    for compress in [false, true] {
        let before = LIVE.load(Relaxed);
        let mut p = vt100::Parser::new(nz(rows), nz(cols), cap);
        p.screen_mut().set_scrollback_compression(compress);
        let t = Instant::now();
        for l in lines {
            p.process(l);
            p.process(b"\r\n");
        }
        let feed = t.elapsed();
        let mem = LIVE.load(Relaxed) - before;
        if !compress {
            base = mem;
        }
        println!(
            "\n{}: kept {} lines; heap {:.2} MiB{}; feed {:.0?}",
            if compress { "compressed  " } else { "uncompressed" },
            p.screen().scrollback_len(),
            mem as f64 / 1048576.0,
            if compress { format!(" ({:.1}x smaller)", base as f64 / mem as f64) } else { String::new() },
            feed
        );
        for q in ["error", "Installer", "2025", "zzzzqq"] {
            PEAK.store(LIVE.load(Relaxed), Relaxed);
            let t = Instant::now();
            let hits = search(&mut p, q);
            let first = t.elapsed();
            let peak = PEAK.load(Relaxed) - before;
            // View returns to the bottom, then a scroll-by-one lets blocks refreeze.
            p.screen_mut().set_scrollback(1);
            p.screen_mut().set_scrollback(0);
            let after = LIVE.load(Relaxed) - before;
            println!(
                "  search {q:10} {hits:6} hits  {first:>8.1?}  peak {:.2} MiB, settled {:.2} MiB",
                peak as f64 / 1048576.0,
                after as f64 / 1048576.0
            );
        }
        // Wheel-scrolling a few pages around the middle (the common interactive case).
        let t = Instant::now();
        for i in 0..2000 {
            p.screen_mut().set_scrollback(cap / 2 + (i % 40));
        }
        println!("  2000 small scroll steps in the middle: {:.1?}", t.elapsed());
        p.screen_mut().set_scrollback(0);
    }
}
