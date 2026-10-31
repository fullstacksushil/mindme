use super::*;

#[test]
fn copy_gives_a_node_and_its_subtree_as_bullets() {
    let m = Map::parse("# Rust\n> A language.\n- Ownership\n  > Who frees memory.\n  - Borrowing\n- Cargo\n").unwrap();
    let ownership = m.layout().nodes[1].id;
    assert_eq!(m.subtree(ownership), "- Ownership\n  > Who frees memory.\n  - Borrowing\n");
}

#[test]
fn paste_adds_lines_as_branches_under_the_current_node_in_one_step() {
    let mut m = Map::new(t("Trip"));
    m.step(vec![Cmd::Paste("- Packing\n  > Light.\n  - Socks\nTickets\n> About the trip.\n".into())]);
    assert_eq!(m.to_markdown(), "# Trip\n> About the trip.\n- Packing\n  > Light.\n  - Socks\n- Tickets\n");
    m.step(vec![Cmd::Undo]);
    assert_eq!(m.to_markdown(), "# Trip\n", "one undo takes the whole paste back");
}

#[test]
fn paste_a_copied_subtree_or_a_whole_map() {
    let source = Map::parse("# Rust\n- Ownership\n  > Who frees memory.\n  - Borrowing\n").unwrap();
    let mut m = Map::new(t("Langs"));
    m.step(vec![Cmd::Paste(source.subtree(source.layout().nodes[1].id)), Cmd::Paste(source.to_markdown())]);
    assert_eq!(m.to_markdown(), "# Langs\n- Ownership\n  > Who frees memory.\n  - Borrowing\n- Rust\n  - Ownership\n    > Who frees memory.\n    - Borrowing\n");
}

#[test]
fn json_export_nests_children_and_leaves_out_missing_notes() {
    let m = Map::parse("# Rust\n> A language.\n- Ownership\n  - Borrowing\n- Cargo\n  > The build tool.\n").unwrap();
    let json: serde_json::Value = serde_json::from_str(&m.to_json()).unwrap();
    let want = serde_json::json!({
        "text": "Rust",
        "note": "A language.",
        "children": [
            { "text": "Ownership", "children": [{ "text": "Borrowing", "children": [] }] },
            { "text": "Cargo", "note": "The build tool.", "children": [] },
        ],
    });
    assert_eq!(json, want);
}
