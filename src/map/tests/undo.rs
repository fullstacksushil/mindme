use super::*;

fn id(m: &Map, text: &str) -> NodeId {
    m.layout().nodes.iter().find(|n| n.text.as_str() == text).unwrap().id
}

#[test]
fn undo_and_redo_a_kept_ghost() {
    let mut m = Map::new(t("Rust"));
    let root = m.current();
    let ask = m.step(vec![]).pop().unwrap();
    m.receive(ask, Ok(ghosts(vec![t("Ownership"), t("Cargo")])));
    assert_eq!(m.step(vec![Cmd::Keep(0)]).len(), 1, "the kept node asks for its ghosts");
    let kept = m.current();
    assert!(m.step(vec![Cmd::Undo]).is_empty(), "the root's ghosts are cached");
    assert_eq!(m.to_markdown(), "# Rust\n");
    assert_eq!(m.current(), root, "undo goes back to the node the step changed");
    assert_eq!(only_ghost(&m), "Cargo");
    assert_eq!(m.step(vec![Cmd::Redo]).len(), 1, "its dropped request is asked afresh, not left loading");
    assert_eq!(m.to_markdown(), "# Rust\n- Ownership\n");
    assert_eq!(m.current(), kept, "redo goes back to where you were, with the same node");
}

#[test]
fn undo_a_branch_added_by_hand_and_a_delete() {
    let mut m = Map::new(t("Rust"));
    let ask = m.step(vec![]).pop().unwrap();
    m.receive(ask, Ok(ghosts(vec![t("Ownership")])));
    m.step(vec![Cmd::SetAuto(false), Cmd::Add(t("Cargo"))]);
    m.step(vec![Cmd::Undo]);
    assert_eq!(m.to_markdown(), "# Rust\n");
    m.step(vec![Cmd::Redo]);
    let cargo = id(&m, "Cargo");
    m.step(vec![Cmd::Select(cargo), Cmd::Delete]);
    assert_eq!(m.to_markdown(), "# Rust\n");
    m.step(vec![Cmd::Undo]);
    assert_eq!(m.to_markdown(), "# Rust\n- Cargo\n");
    assert_eq!(m.current(), cargo, "the deleted leaf is back and current");
    m.step(vec![Cmd::Back]);
    assert_eq!(only_ghost(&m), "Ownership", "the deleted text is no longer offered as a ghost");
}

#[test]
fn undo_a_rename_keeps_the_note_and_drops_the_new_name_s_reply() {
    let mut m = Map::parse("# Rust\n> Mine.\n").unwrap();
    m.step(vec![]);
    let go = m.step(vec![Cmd::Rename(m.current(), t("Go"))]).pop().unwrap();
    let again = m.step(vec![Cmd::Undo]).pop().expect("the old name asks afresh");
    assert_eq!(m.to_markdown(), "# Rust\n> Mine.\n");
    m.receive(go, Ok(ghosts(vec![t("Goroutines")])));
    assert!(m.layout().ghosts.iter().all(|s| matches!(s, Slot::Shimmer)), "a reply for the undone name is dropped");
    m.receive(again, Ok(ghosts(vec![t("Cargo")])));
    assert_eq!(only_ghost(&m), "Cargo");
    m.step(vec![Cmd::Redo]);
    assert_eq!(m.to_markdown(), "# Go\n> Mine.\n");
}

#[test]
fn note_edits_undo_one_at_a_time_on_the_edited_node() {
    let mut m = Map::parse("# Rust\n- Cargo\n").unwrap();
    m.step(vec![Cmd::SetAuto(false)]);
    let cargo = id(&m, "Cargo");
    m.step(vec![Cmd::SetNote(cargo, "Builds.".into()), Cmd::SetNote(cargo, "Builds code.".into())]);
    m.step(vec![Cmd::Undo]);
    assert_eq!(m.to_markdown(), "# Rust\n- Cargo\n  > Builds.\n");
    assert_eq!(m.current(), cargo, "a note saved after leaving its node still undoes there");
    m.step(vec![Cmd::Undo]);
    assert_eq!(m.to_markdown(), "# Rust\n- Cargo\n");
}

#[test]
fn a_map_out_is_one_step() {
    let mut m = Map::parse("# Ideas\n- Rust\n").unwrap();
    m.step(vec![Cmd::SetAuto(false)]);
    let rust = id(&m, "Rust");
    let ask = m.step(vec![Cmd::Select(rust), Cmd::LearnMore(Focus::MapOut)]).pop().unwrap();
    m.receive(ask, Ok(Answer { note: Some("# Rust\n> A language.\n- Cargo\n  - Crates\n- Ownership\n".into()), ghosts: vec![] }));
    let grown = "# Ideas\n- Rust\n  > A language.\n  - Cargo\n    - Crates\n  - Ownership\n";
    assert_eq!(m.to_markdown(), grown);
    m.step(vec![Cmd::Select(id(&m, "Crates")), Cmd::Undo]);
    assert_eq!(m.to_markdown(), "# Ideas\n- Rust\n");
    assert_eq!(m.current(), rust);
    m.step(vec![Cmd::Redo]);
    assert_eq!(m.to_markdown(), grown);
}

#[test]
fn a_new_edit_clears_redo_and_navigation_is_not_a_step() {
    let mut m = Map::parse("# Rust\n- Cargo\n").unwrap();
    let cargo = id(&m, "Cargo");
    m.step(vec![Cmd::SetAuto(false), Cmd::Add(t("Traits")), Cmd::Undo, Cmd::Add(t("Macros")), Cmd::Redo]);
    assert_eq!(m.to_markdown(), "# Rust\n- Cargo\n- Macros\n");
    m.step(vec![Cmd::Select(cargo), Cmd::Back, Cmd::Reroll, Cmd::SetAuto(true), Cmd::Select(cargo), Cmd::Undo]);
    assert_eq!(m.to_markdown(), "# Rust\n- Cargo\n", "one undo skips the navigation and takes back the add");
    m.step(vec![Cmd::Undo]);
    assert_eq!(m.to_markdown(), "# Rust\n- Cargo\n", "nothing older to undo");
}

#[test]
fn a_note_filled_by_a_ghost_reply_is_not_a_step_and_survives_undo() {
    let mut m = Map::new(t("Rust"));
    m.step(vec![Cmd::SetAuto(false), Cmd::Add(t("Cargo")), Cmd::Back]);
    let ask = m.step(vec![Cmd::Reroll]).pop().unwrap();
    m.receive(ask, Ok(Answer { note: Some("A language.".into()), ghosts: vec![t("Traits")] }));
    m.step(vec![Cmd::Undo]);
    assert_eq!(m.to_markdown(), "# Rust\n> A language.\n", "the undo took back the branch and kept the note");
    m.step(vec![Cmd::Redo]);
    assert_eq!(m.to_markdown(), "# Rust\n> A language.\n- Cargo\n");
}

#[test]
fn a_reply_for_an_undone_node_is_dropped_and_redo_leaves_nothing_loading() {
    let mut m = Map::new(t("Rust"));
    m.step(vec![Cmd::SetAuto(false), Cmd::Add(t("Cargo"))]);
    let ask = m.step(vec![Cmd::LearnMore(Focus::Depth)]).pop().unwrap();
    m.step(vec![Cmd::Undo]);
    m.receive(ask, Ok(Answer { note: Some("Builds.".into()), ghosts: vec![] }));
    m.step(vec![Cmd::Redo]);
    assert_eq!(m.to_markdown(), "# Rust\n- Cargo\n");
    assert_eq!(m.layout().learning, None, "Learn more is not stuck loading");
}

#[test]
fn ids_are_never_reused() {
    let mut m = Map::new(t("Rust"));
    m.step(vec![Cmd::SetAuto(false), Cmd::Add(t("Cargo"))]);
    let cargo = m.current();
    m.step(vec![Cmd::Undo, Cmd::Redo]);
    assert_eq!(id(&m, "Cargo"), cargo, "redo brings back the same node");
    m.step(vec![Cmd::Undo, Cmd::Add(t("Traits"))]);
    assert_ne!(m.current(), cargo, "a new node never takes an undone node's id");
}

#[test]
fn stop_gives_up_each_wait_and_drops_its_late_reply() {
    let mut m = Map::new(t("Rust"));
    let ghosts_ask = m.step(vec![]).pop().unwrap();
    let learn = m.step(vec![Cmd::LearnMore(Focus::Depth)]).pop().unwrap();
    let chat = m.step(vec![Cmd::Chat("Why?".into())]).pop().unwrap();
    assert!(m.step(vec![Cmd::Stop(Wait::Ghosts), Cmd::Stop(Wait::Learn), Cmd::Stop(Wait::Chat)]).is_empty(), "a stop asks nothing");
    let layout = m.layout();
    assert!(matches!(layout.ghosts[..], [Slot::Retry("stopped")]));
    assert_eq!(layout.learning, Some(Some("stopped")));
    assert_eq!(layout.chat.last(), Some(&Turn::Failed("stopped".into())));
    for ask in [ghosts_ask, learn, chat] {
        m.receive(ask, Ok(Answer { note: Some("Late.".into()), ghosts: vec![t("Late")] }));
    }
    assert_eq!(m.to_markdown(), "# Rust\n", "every late reply was dropped");
    assert_eq!(m.layout().chat.len(), 2);
}

#[test]
fn delete_takes_the_branches_with_it_and_undo_brings_them_back() {
    let mut m = Map::parse("# Rust\n- Ownership\n  - Borrowing\n    - Lifetimes\n- Cargo\n").unwrap();
    m.step(vec![Cmd::SetAuto(false), Cmd::Select(id(&m, "Ownership")), Cmd::Delete]);
    assert_eq!(m.to_markdown(), "# Rust\n- Cargo\n");
    assert_eq!(m.current(), id(&m, "Rust"));
    m.step(vec![Cmd::Undo]);
    assert_eq!(m.to_markdown(), "# Rust\n- Ownership\n  - Borrowing\n    - Lifetimes\n- Cargo\n");
    m.step(vec![Cmd::Select(id(&m, "Rust")), Cmd::Delete]);
    assert_eq!(m.layout().nodes.len(), 5, "the root stays");
}

#[test]
fn moving_a_node_carries_its_branches_and_refuses_loops() {
    let mut m = Map::parse("# Rust\n- Ownership\n  - Borrowing\n- Cargo\n- Traits\n").unwrap();
    m.step(vec![Cmd::SetAuto(false)]);
    let (ownership, traits, borrowing) = (id(&m, "Ownership"), id(&m, "Traits"), id(&m, "Borrowing"));
    m.step(vec![Cmd::Move { node: ownership, parent: traits, before: None }]);
    assert_eq!(m.to_markdown(), "# Rust\n- Cargo\n- Traits\n  - Ownership\n    - Borrowing\n");
    assert_eq!(m.current(), ownership, "the moved node is current");
    m.step(vec![Cmd::Undo]);
    assert_eq!(m.to_markdown(), "# Rust\n- Ownership\n  - Borrowing\n- Cargo\n- Traits\n", "one undo step");
    m.step(vec![Cmd::Move { node: traits, parent: id(&m, "Rust"), before: Some(ownership) }]);
    assert_eq!(m.to_markdown(), "# Rust\n- Traits\n- Ownership\n  - Borrowing\n- Cargo\n", "reordered before a sibling");
    let before = m.to_markdown();
    for refused in [
        Cmd::Move { node: ownership, parent: borrowing, before: None },
        Cmd::Move { node: ownership, parent: ownership, before: None },
        Cmd::Move { node: id(&m, "Rust"), parent: id(&m, "Cargo"), before: None },
    ] {
        m.step(vec![refused]);
        assert_eq!(m.to_markdown(), before, "into its own branches, onto itself, or the root does nothing");
    }
    m.step(vec![Cmd::Move { node: traits, parent: id(&m, "Rust"), before: None }]);
    assert_eq!(m.to_markdown(), "# Rust\n- Ownership\n  - Borrowing\n- Cargo\n- Traits\n", "no place given moves it after the last sibling");
}
