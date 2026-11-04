#![cfg(feature = "compress")]
//! Compressed scrollback reads exactly like plain scrollback.

fn nz(n: u16) -> core::num::NonZero<u16> {
    core::num::NonZero::new(n).unwrap()
}

fn feed(p: &mut vt100::Parser, lines: usize) {
    for i in 0..lines {
        let line = format!("\x1b[3{}mline {i} \x1b[1;48;2;1;2;3mbold é 世\x1b[0m\r\n", i % 8);
        p.process(line.as_bytes());
    }
}

fn dump(p: &mut vt100::Parser, offsets: &[usize]) -> Vec<String> {
    offsets
        .iter()
        .map(|&o| {
            p.screen_mut().set_scrollback(o);
            p.screen().contents_formatted()
        })
        .collect()
}

#[test]
fn compressed_matches_plain() {
    let mut plain = vt100::Parser::new(nz(10), nz(40), 3000);
    let mut packed = vt100::Parser::new(nz(10), nz(40), 3000);
    packed.screen_mut().set_scrollback_compression(true);
    for _ in 0..3 {
        feed(&mut plain, 2500);
        feed(&mut packed, 2500);
        let offs = [0, 5, 700, 1500, 2990, 0, 3000, 100, 0];
        assert_eq!(plain.screen().scrollback_len(), packed.screen().scrollback_len());
        assert_eq!(dump(&mut plain, &offs), dump(&mut packed, &offs));
        // Output while scrolled back keeps the view on its text.
        plain.screen_mut().set_scrollback(1200);
        packed.screen_mut().set_scrollback(1200);
        feed(&mut plain, 50);
        feed(&mut packed, 50);
        assert_eq!(plain.screen().scrollback(), packed.screen().scrollback());
        assert_eq!(dump(&mut plain, &[1250, 0, 2000]), dump(&mut packed, &[1250, 0, 2000]));
    }
    assert_eq!(plain.screen().scrollback_len(), 3000);
}

#[test]
fn toggling_off_and_clearing() {
    let mut p = vt100::Parser::new(nz(10), nz(40), 3000);
    p.screen_mut().set_scrollback_compression(true);
    feed(&mut p, 2000);
    let before = dump(&mut p, &[1500]);
    p.screen_mut().set_scrollback_compression(false);
    assert!(!p.screen().scrollback_compressed());
    assert_eq!(before, dump(&mut p, &[1500]));
    p.screen_mut().set_scrollback_compression(true);
    feed(&mut p, 2000);
    p.process(b"\x1b[3J");
    assert_eq!(p.screen().scrollback_len(), 0);
}

#[test]
fn prompt_marks_survive_compression() {
    let mut plain = vt100::Parser::new(nz(10), nz(40), 3000);
    let mut packed = vt100::Parser::new(nz(10), nz(40), 3000);
    packed.screen_mut().set_scrollback_compression(true);
    for p in [&mut plain, &mut packed] {
        feed(p, 10);
        p.screen_mut().mark_prompt();
        feed(p, 2000);
        assert!(p.screen_mut().clear_before_prompt());
    }
    assert_eq!(plain.screen().scrollback_len(), packed.screen().scrollback_len());
    assert_eq!(dump(&mut plain, &[0, 500, 1990]), dump(&mut packed, &[0, 500, 1990]));
}
