mod helpers;

/// Writes `text` after marking the cursor row as a prompt.
fn prompt(parser: &mut vt100::Parser, text: &str) {
    parser.screen_mut().mark_prompt();
    parser.process(text.as_bytes());
}

#[test]
fn no_mark_changes_nothing() {
    let mut parser = helpers::new(4, 10, 10);
    parser.process(b"1\r\n2\r\n3\r\n4\r\n5\r\n6");
    let before = parser.screen().contents();
    assert!(!parser.screen_mut().clear_before_prompt());
    assert_eq!(parser.screen().contents(), before);
    assert_eq!(parser.screen().scrollback_len(), 2);
}

#[test]
fn mark_on_screen_drops_scrollback_and_rows_above() {
    let mut parser = helpers::new(4, 10, 10);
    parser.process(b"1\r\n2\r\n3\r\n");
    prompt(&mut parser, "$ ls\r\nout");
    // rows: 2 3 "$ ls" out; "1" is in the scrollback
    assert_eq!(parser.screen().scrollback_len(), 1);
    assert_eq!(parser.screen().cursor_position(), (3, 3));
    assert!(parser.screen_mut().clear_before_prompt());
    assert_eq!(parser.screen().scrollback_len(), 0);
    assert_eq!(parser.screen().contents(), "$ ls\nout");
    // the cursor keeps its place relative to the marked row
    assert_eq!(parser.screen().cursor_position(), (1, 3));
    assert_eq!(parser.screen().cell(0, 0).unwrap().contents(), "$");
}

#[test]
fn mark_in_scrollback_keeps_it_and_what_follows() {
    let mut parser = helpers::new(4, 10, 20);
    parser.process(b"a\r\nb\r\n");
    prompt(&mut parser, "$ one\r\n");
    parser.process(b"x\r\ny\r\nz\r\nw\r\nv");
    let len = parser.screen().scrollback_len();
    assert!(len >= 4);
    assert!(parser.screen_mut().clear_before_prompt());
    // "a" and "b" are gone; the marked row is the first scrollback row
    assert_eq!(parser.screen().scrollback_len(), len - 2);
    parser.screen_mut().set_scrollback(usize::MAX);
    assert_eq!(parser.screen().cell(0, 0).unwrap().contents(), "$");
    parser.screen_mut().set_scrollback(0);
    assert_eq!(parser.screen().contents(), "y\nz\nw\nv");
}

#[test]
fn only_the_last_mark_counts() {
    let mut parser = helpers::new(4, 10, 20);
    prompt(&mut parser, "$ first\r\nout1\r\n");
    prompt(&mut parser, "$ second\r\nout2\r\n");
    parser.process(b"p\r\nq");
    assert!(parser.screen_mut().clear_before_prompt());
    assert_eq!(parser.screen().contents(), "$ second\nout2\np\nq");
    assert_eq!(parser.screen().scrollback_len(), 0);
}

#[test]
fn mark_trimmed_by_capacity_is_gone() {
    let mut parser = helpers::new(2, 10, 2);
    prompt(&mut parser, "$ old");
    parser.process(b"\r\n1\r\n2\r\n3\r\n4\r\n5");
    assert_eq!(parser.screen().scrollback_len(), 2);
    assert!(!parser.screen_mut().clear_before_prompt());
}

#[test]
fn mark_survives_resize() {
    let mut parser = helpers::new(4, 10, 20);
    prompt(&mut parser, "$ p\r\n");
    parser.process(b"1\r\n2\r\n3\r\n4");
    // smaller: the marked row is pushed into the scrollback; taller: pulled back
    helpers::set_parser_size(&mut parser, 3, 12);
    helpers::set_parser_size(&mut parser, 8, 10);
    assert!(parser.screen_mut().clear_before_prompt());
    assert_eq!(parser.screen().contents(), "$ p\n1\n2\n3\n4");
}

#[test]
fn clearing_the_row_removes_the_mark() {
    let mut parser = helpers::new(4, 10, 10);
    prompt(&mut parser, "$ a\r\nb");
    parser.process(b"\x1b[1;1H\x1b[2K");
    assert!(!parser.screen_mut().clear_before_prompt());

    let mut parser = helpers::new(4, 10, 10);
    prompt(&mut parser, "$ a");
    parser.process(b"\x1b[2J");
    assert!(!parser.screen_mut().clear_before_prompt());
}

#[test]
fn mark_on_the_first_row_drops_only_the_scrollback() {
    let mut parser = helpers::new(3, 10, 10);
    parser.process(b"1\r\n2\r\n3\r\n");
    prompt(&mut parser, "$ a\r\nb");
    assert_eq!(parser.screen().contents(), "3\n$ a\nb");
    parser.process(b"\r\nc");
    // rows: "$ a" b c, the mark on row 0
    assert_eq!(parser.screen().scrollback_len(), 3);
    assert!(parser.screen_mut().clear_before_prompt());
    assert_eq!(parser.screen().scrollback_len(), 0);
    assert_eq!(parser.screen().contents(), "$ a\nb\nc");
    assert_eq!(parser.screen().cursor_position(), (2, 1));
}

#[test]
fn clearing_returns_to_the_live_screen() {
    // mark on the screen
    let mut parser = helpers::new(3, 10, 10);
    parser.process(b"1\r\n2\r\n3\r\n4\r\n");
    prompt(&mut parser, "$ a");
    parser.screen_mut().set_scrollback(2);
    assert_eq!(parser.screen().scrollback(), 2);
    assert!(parser.screen_mut().clear_before_prompt());
    assert_eq!(parser.screen().scrollback(), 0);
    assert_eq!(parser.screen().contents(), "$ a");

    // mark in the scrollback
    let mut parser = helpers::new(2, 10, 10);
    parser.process(b"1\r\n2\r\n");
    prompt(&mut parser, "$ a\r\n");
    parser.process(b"x\r\ny\r\nz");
    parser.screen_mut().set_scrollback(3);
    assert_eq!(parser.screen().scrollback(), 3);
    assert!(parser.screen_mut().clear_before_prompt());
    assert_eq!(parser.screen().scrollback(), 0);
    assert_eq!(parser.screen().scrollback_len(), 2);
    assert_eq!(parser.screen().contents(), "y\nz");
}

#[test]
fn trimmed_rows_above_the_mark_are_not_pulled_back_as_blanks() {
    // 10 rows scrolled through a scrollback of 3: the screen grows by 4
    // after the clear, and only the marked row is pulled back above it
    let mut parser = helpers::new(2, 10, 3);
    for i in 0..10 {
        parser.process(format!("{i}\r\n").as_bytes());
    }
    prompt(&mut parser, "$ p\r\n");
    parser.process(b"a\r\nb");
    assert!(parser.screen_mut().clear_before_prompt());
    assert_eq!(parser.screen().scrollback_len(), 1);
    helpers::set_parser_size(&mut parser, 6, 10);
    assert_eq!(parser.screen().cell(0, 0).unwrap().contents(), "$");
    assert_eq!(parser.screen().contents(), "$ p\na\nb");
    assert_eq!(parser.screen().cursor_position(), (2, 1));
}

#[test]
fn the_saved_cursor_moves_up_with_the_rows() {
    let mut parser = helpers::new(5, 10, 0);
    parser.process(b"a\r\n");
    prompt(&mut parser, "$ x\r\no1\r\no2");
    // save at (3, 2), then move to row 4
    parser.process(b"\x1b7\r\nz");
    assert!(parser.screen_mut().clear_before_prompt());
    assert_eq!(parser.screen().cursor_position(), (3, 1));
    parser.process(b"\x1b8");
    assert_eq!(parser.screen().cursor_position(), (2, 2));
    assert_eq!(parser.screen().contents(), "$ x\no1\no2\nz");
}

#[test]
fn a_cursor_above_the_mark_stops_at_the_top() {
    let mut parser = helpers::new(4, 10, 0);
    parser.process(b"a\r\nb\r\n");
    prompt(&mut parser, "$ x");
    parser.process(b"\x1b[1;5H");
    assert!(parser.screen_mut().clear_before_prompt());
    assert_eq!(parser.screen().cursor_position(), (0, 4));
    assert_eq!(parser.screen().contents(), "$ x");
}

#[test]
fn the_alternate_screen_has_its_own_marks() {
    let mut parser = helpers::new(3, 10, 10);
    parser.process(b"1\r\n");
    prompt(&mut parser, "$ vim");
    parser.process(b"\x1b[?1049h");
    // the primary screen's mark isn't the alternate screen's
    assert!(!parser.screen_mut().clear_before_prompt());
    parser.process(b"\x1b[?1049l");
    assert!(parser.screen_mut().clear_before_prompt());
    assert_eq!(parser.screen().contents(), "$ vim");
}

#[test]
fn a_scroll_region_doesnt_get_in_the_way() {
    let mut parser = helpers::new(4, 10, 10);
    parser.process(b"a\r\n");
    prompt(&mut parser, "$ x\r\no");
    parser.process(b"\x1b[2;3r\x1b[3;1H");
    assert!(parser.screen_mut().clear_before_prompt());
    assert_eq!(parser.screen().contents(), "$ x\no");
    assert_eq!(parser.screen().cursor_position(), (1, 0));
    // the region still scrolls without panicking
    parser.process(b"\r\n\r\n\r\nq");
}
