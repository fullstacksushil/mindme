use super::*;

mod clipboard;
mod undo;

fn t(s: &str) -> Text {
    Text::new(s).unwrap()
}

fn ghosts(ghosts: Vec<Text>) -> Answer {
    Answer { note: None, ghosts }
}

fn only_ghost(m: &Map) -> String {
    match &m.layout().ghosts[..] {
        [Slot::Ghost(0, g)] => g.as_str().to_owned(),
        _ => panic!("expected exactly one ghost"),
    }
}

#[test]
fn prompt_matches_spec_wording() {
    let mut m = Map::new(t("Learn Rust"));
    let ask = m.step(vec![]).pop().unwrap();
    m.receive(ask, Ok(ghosts(vec![t("Ownership")])));
    let ask = m.step(vec![Cmd::Keep(0)]).pop().unwrap();
    assert_eq!(
        ask.prompt(),
        "Mind map path: Learn Rust > Ownership\nReply with only a JSON object. \"note\" explains \"Ownership\" in 2 to 3 sentences. \"nodes\" is an array of 5 new child nodes, 1 to 4 words each.\nFormat: {\"note\": \"...\", \"nodes\": [\"...\", \"...\"]}"
    );
}

#[test]
fn markdown_round_trip() {
    let md = "# Learn Rust\n- Ownership\n  - Borrowing\n  - #hashtag\n- Tooling\n  - cargo\n";
    assert_eq!(Map::parse(md).unwrap().to_markdown(), md);
}

#[test]
fn markdown_hand_edits() {
    let want = "# Root\n- a\n  - b\n  - c\n- d\n";
    for md in [
        "# Root\n- a\n\t- b\n\t- c\n- d\n",
        "# Root\n* a\n    + b\n    - c\n- d\n",
        "# Root\n- a\n   - b\n  - c\n- d\n",
        "notes\n# Root\n\n- a\n  - b\n  - c\n- d\n",
    ] {
        assert_eq!(Map::parse(md).unwrap().to_markdown(), want, "{md:?}");
    }
}

#[test]
fn stale_reply_is_dropped() {
    let mut m = Map::new(t("Rust"));
    let old = m.step(vec![]).pop().unwrap();
    let new = m.step(vec![Cmd::Rename(m.current(), t("Go"))]).pop().unwrap();
    m.receive(old, Ok(ghosts(vec![t("Borrowing")])));
    assert!(m.layout().ghosts.iter().all(|s| matches!(s, Slot::Shimmer)));
    m.receive(new, Ok(ghosts(vec![t("Goroutines")])));
    assert_eq!(only_ghost(&m), "Goroutines");
}

#[test]
fn rename_keeps_the_note_and_drops_the_ghosts() {
    let mut m = Map::parse("# Rust\n> Mine.\n").unwrap();
    let ask = m.step(vec![]).pop().unwrap();
    m.receive(ask, Ok(ghosts(vec![t("Borrowing")])));
    let ask = m.step(vec![Cmd::Rename(m.current(), t("Go"))]).pop().unwrap();
    assert_eq!(m.to_markdown(), "# Go\n> Mine.\n", "the note you wrote survives a rename");
    assert!(m.layout().ghosts.iter().all(|s| matches!(s, Slot::Shimmer)), "the old ghosts are gone and new ones are on the way");
    assert!(ask.prompt().starts_with("Mind map path: Go\n"));
}

#[test]
fn reroll_filters_rejected_and_children() {
    let mut m = Map::new(t("Rust"));
    let ask = m.step(vec![]).pop().unwrap();
    m.receive(ask, Ok(ghosts(vec![t("Ownership"), t("Cargo")])));
    m.step(vec![Cmd::Keep(0), Cmd::Back]);
    let ask = m.step(vec![Cmd::Reroll]).pop().unwrap();
    m.receive(ask, Ok(ghosts(vec![t("cargo"), t("OWNERSHIP"), t("Traits")])));
    assert_eq!(only_ghost(&m), "Traits");
}

#[test]
fn delete_puts_the_text_back_as_a_ghost() {
    let mut m = Map::new(t("Rust"));
    let ask = m.step(vec![]).pop().unwrap();
    m.receive(ask, Ok(ghosts(vec![t("Ownership")])));
    m.step(vec![Cmd::Keep(0)]);
    assert!(m.step(vec![Cmd::Delete]).is_empty(), "the parent's ghosts are cached");
    assert_eq!(m.current(), NodeId(0));
    assert_eq!(only_ghost(&m), "Ownership");
    assert_eq!(m.to_markdown(), "# Rust\n");
}

#[test]
fn markdown_notes_round_trip_and_keep_hand_written_prose() {
    let md = "# Root\n> Root **note**.\n- a\n  > Uses `&T`:\n  >\n  > - not a branch\n  > # not a title\n  - b\n    > Note for b.\n- c\n";
    let map = Map::parse(md).unwrap();
    assert_eq!(map.to_markdown(), md);
    assert_eq!(map.layout().nodes.len(), 4, "list lines inside a note are not branches");
    let hand = "# Root\n- a\n  first line\n  second line\n- b\nprose line\n";
    assert_eq!(Map::parse(hand).unwrap().to_markdown(), "# Root\n> prose line\n- a\n  > first line\n  > second line\n- b\n");
}

#[test]
fn set_note_edits_and_clears() {
    let mut m = Map::new(t("Rust"));
    m.step(vec![Cmd::SetNote(m.current(), "  Line one\n\nLine two  ".into())]);
    assert_eq!(m.to_markdown(), "# Rust\n> Line one\n>\n> Line two\n");
    m.step(vec![Cmd::SetNote(m.current(), " \n ".into())]);
    assert_eq!(m.to_markdown(), "# Rust\n");
}

#[test]
fn a_topic_outline_parses_into_a_whole_map() {
    assert!(topic_prompt("Rust", None).contains("# Rust\n> what it is, 1 to 2 sentences\n> Sources: [title](url), ...\n"));
    let md = "# Rust\n> A language.\n> Sources: [Rust](https://rust-lang.org), [Book](https://doc.rust-lang.org/book/)\n- Ownership\n  > Who frees memory.\n  - Borrowing\n- Cargo\n";
    let m = Map::parse(md).unwrap();
    assert_eq!(m.layout().nodes.len(), 4, "a Sources line is never a branch");
    assert!(matches!(m.layout().note, Some(Note::Text("A language.\nSources: [Rust](https://rust-lang.org), [Book](https://doc.rust-lang.org/book/)"))));
    assert_eq!(m.to_markdown(), md);
}

#[test]
fn ancestors_walk_up_to_the_root() {
    let m = Map::parse("# Rust\n- Ownership\n  - Borrowing\n- Cargo\n").unwrap();
    let layout = m.layout();
    let id = |text: &str| layout.nodes.iter().find(|n| n.text.as_str() == text).unwrap().id;
    let names = |from: &str| layout.ancestors(id(from)).map(|a| layout.node(a).unwrap().text.as_str()).collect::<Vec<_>>();
    assert_eq!(names("Borrowing"), ["Ownership", "Rust"]);
    assert!(names("Rust").is_empty());
    assert_eq!(layout.children(id("Rust")).map(|n| n.text.as_str()).collect::<Vec<_>>(), ["Ownership", "Cargo"]);
}

#[test]
fn one_reply_brings_ghosts_and_note() {
    let mut m = Map::new(t("Rust"));
    let ask = m.step(vec![]).pop().unwrap();
    assert!(matches!(m.layout().note, Some(Note::Loading)));
    m.receive(ask, Ok(Answer { note: Some(" A systems language.\n".into()), ghosts: vec![t("Cargo")] }));
    assert!(matches!(m.layout().note, Some(Note::Text("A systems language."))));
    assert_eq!(only_ghost(&m), "Cargo");
    let ask = m.step(vec![Cmd::Reroll]).pop().unwrap();
    m.receive(ask, Ok(Answer { note: Some("Different.".into()), ghosts: vec![t("Traits")] }));
    assert_eq!(m.to_markdown(), "# Rust\n> A systems language.\n", "More ideas keeps the first note");
}

#[test]
fn with_auto_off_only_reroll_asks() {
    let mut m = Map::parse("# Rust\n- Ownership\n").unwrap();
    assert!(m.step(vec![Cmd::SetAuto(false)]).is_empty());
    let child = m.layout().nodes[1].id;
    assert!(m.step(vec![Cmd::Select(child)]).is_empty(), "opening a node asks nothing");
    assert!(m.layout().ghosts.is_empty() && m.layout().note.is_none());
    let ask = m.step(vec![Cmd::Reroll]).pop().expect("More ideas asks");
    m.receive(ask, Ok(ghosts(vec![t("Moves")])));
    assert_eq!(only_ghost(&m), "Moves");
}

#[test]
fn with_auto_off_keeping_ghosts_stays_on_the_parent() {
    let mut m = Map::new(t("Rust"));
    m.step(vec![Cmd::SetAuto(false)]);
    let root = m.current();
    let ask = m.step(vec![Cmd::Reroll]).pop().unwrap();
    m.receive(ask, Ok(ghosts(vec![t("Ownership"), t("Cargo"), t("Traits")])));
    assert!(m.step(vec![Cmd::Keep(0)]).is_empty(), "keeping asks nothing");
    assert_eq!(m.current(), root, "the parent stays current");
    m.step(vec![Cmd::Keep(1)]);
    assert_eq!(m.to_markdown(), "# Rust\n- Ownership\n- Traits\n");
    assert_eq!(only_ghost(&m), "Cargo", "the rest stay showing");
}

#[test]
fn add_by_hand_builds_a_chain_without_asking() {
    let mut m = Map::new(t("Rust"));
    assert!(m.step(vec![Cmd::SetAuto(false), Cmd::Add(t("Ownership")), Cmd::Add(t("Borrowing"))]).is_empty());
    m.step(vec![Cmd::Back, Cmd::Add(t("Cargo"))]);
    assert_eq!(m.to_markdown(), "# Rust\n- Ownership\n  - Borrowing\n  - Cargo\n");
    assert_eq!(m.layout().nodes.last().unwrap().id, m.current(), "the added node is current");
}

#[test]
fn learn_more_adds_under_the_note_and_drops_stale_replies() {
    let mut m = Map::parse("# Rust\n> A language.\n").unwrap();
    m.step(vec![Cmd::SetAuto(false)]);
    let ask = m.step(vec![Cmd::LearnMore(Focus::Depth)]).pop().unwrap();
    assert!(ask.prompt().starts_with("Mind map path: Rust\nAlready noted:\nA language.\nExplain \"Rust\" in depth"));
    assert!(m.step(vec![Cmd::LearnMore(Focus::Examples)]).is_empty(), "one at a time, whatever it asks for");
    assert_eq!(m.layout().learning, Some(None));
    m.receive(ask, Ok(Answer { note: Some("## Ownership\nNo GC.".into()), ghosts: vec![] }));
    assert_eq!(m.to_markdown(), "# Rust\n> A language.\n>\n> ## Ownership\n> No GC.\n");
    assert_eq!(m.layout().learning, None);

    let stale = m.step(vec![Cmd::LearnMore(Focus::Examples)]).pop().unwrap();
    assert_eq!(
        stale.prompt(),
        "Mind map path: Rust\nAlready noted:\nA language.\n\n## Ownership\nNo GC.\nGive 2 to 3 concrete examples of \"Rust\" as a Markdown \"## Examples\" section. \
Add to what is already noted, never repeat it. End with a \"Sources\" list of links you are sure exist. Reply with only the Markdown."
    );
    m.step(vec![Cmd::Rename(m.current(), t("Go"))]);
    m.receive(stale, Ok(Answer { note: Some("About Rust.".into()), ghosts: vec![] }));
    assert_eq!(m.to_markdown(), "# Go\n> A language.\n>\n> ## Ownership\n> No GC.\n", "a reply for the old name is dropped");
}

#[test]
fn map_out_merges_an_outline_under_the_node() {
    let mut m = Map::parse("# Ideas\n- Rust\n  > Mine.\n  - Cargo\n").unwrap();
    m.step(vec![Cmd::SetAuto(false)]);
    let rust = m.layout().nodes[1].id;
    let ask = m.step(vec![Cmd::Select(rust), Cmd::LearnMore(Focus::MapOut)]).pop().unwrap();
    assert!(ask.markdown());
    assert!(ask.prompt().starts_with("Mind map path: Ideas > Rust\nMap out \"Rust\""), "no note goes along: {}", ask.prompt());
    let outline = "Here it is:\n# Rust\n> A language.\n- cargo\n  > The build tool.\n  - Crates\n- Ownership\n  > Who frees memory.\n";
    m.receive(ask, reply(outline));
    assert_eq!(
        m.to_markdown(),
        "# Ideas\n- Rust\n  > Mine.\n  - Cargo\n    > The build tool.\n    - Crates\n  - Ownership\n    > Who frees memory.\n",
        "same-named branches merge, empty notes fill, written notes stay"
    );
    assert_eq!(m.layout().learning, None);

    let ask = m.step(vec![Cmd::LearnMore(Focus::MapOut)]).pop().unwrap();
    m.receive(ask, reply("Sorry, I can't."));
    assert_eq!(m.layout().learning, Some(Some("the reply had no '# title' line")));
}

#[test]
fn the_interview_brief_is_saved_hidden_and_sent_with_every_request() {
    let turns = [("Why Rust?".to_owned(), "Systems work.".to_owned()), ("Level?".to_owned(), "Beginner".to_owned())];
    assert_eq!(brief(&turns), "Q: Why Rust?\nA: Systems work.\nQ: Level?\nA: Beginner");
    assert!(interview_prompt("Rust", false, &[]).contains("a brainstorm grown one idea at a time"));
    assert!(interview_prompt("Rust", true, &turns).contains("a whole map made in one go. Ask one question at a time"));
    assert!(interview_prompt("Rust", true, &turns).contains("So far:\nQ: Why Rust?\nA: Systems work.\nQ: Level?\nA: Beginner\nReply with only a JSON object"));
    assert!(topic_prompt("Rust", Some("Q: Level?\nA: Beginner")).starts_with("About this map, from an interview:\nQ: Level?\nA: Beginner\nMap out \"Rust\" completely"));

    let mut m = Map::new(t("Rust"));
    m.set_brief(Some(format!("{}\n", brief(&turns))));
    let ask = m.step(vec![]).pop().unwrap();
    assert!(ask.prompt().starts_with("Mind map path: Rust\nAbout this map, from an interview:\nQ: Why Rust?\nA: Systems work.\nQ: Level?\nA: Beginner\nReply"));
    m.receive(ask, Ok(Answer { note: Some("A language.".into()), ghosts: vec![t("Cargo")] }));
    m.step(vec![Cmd::Keep(0)]);
    let md = m.to_markdown();
    assert_eq!(md, "# Rust\n<!-- brief\nQ: Why Rust?\nA: Systems work.\nQ: Level?\nA: Beginner\n-->\n> A language.\n- Cargo\n");
    let back = Map::parse(&md).unwrap();
    assert_eq!(back.layout().brief, Some("Q: Why Rust?\nA: Systems work.\nQ: Level?\nA: Beginner"));
    assert_eq!(back.to_markdown(), md, "it round-trips");
    assert_eq!(Map::parse("# A\n> x\n").unwrap().layout().brief, None);

    m.set_brief(Some("Q: a --> b".into()));
    assert!(Map::parse(&m.to_markdown()).unwrap().layout().nodes.len() == 2, "an arrow in an answer cannot end the comment early");
}

fn reply(text: &str) -> Result<Answer, String> {
    Ok(Answer { note: Some(text.into()), ghosts: vec![] })
}

#[test]
fn chat_answers_one_message_at_a_time_and_drops_stale_replies() {
    let mut m = Map::parse("# Rust\n> A language.\n").unwrap();
    m.step(vec![Cmd::SetAuto(false)]);
    assert!(m.step(vec![Cmd::Chat("  ".into())]).is_empty(), "an empty message sends nothing");
    let mut asks = m.step(vec![Cmd::Chat(" Why no GC? ".into()), Cmd::Chat("And?".into())]);
    assert_eq!(asks.len(), 1, "the second waits for the first reply");
    let ask = asks.pop().unwrap();
    assert!(ask.markdown());
    assert_eq!(
        ask.prompt(),
        "Mind map path: Rust\nAlready noted:\nA language.\nYou: Why no GC?\nReply to the last message about \"Rust\", briefly, in Markdown. When it states facts, end with a \"Sources\" list of links you are sure exist. Reply with only the Markdown."
    );
    m.receive(ask, Err("timed out".into()));
    assert_eq!(m.layout().chat, [Turn::You("Why no GC?".into()), Turn::Failed("timed out".into())]);
    let ask = m.step(vec![Cmd::Chat("Why no GC?".into())]).pop().expect("a failure can be resent");
    assert!(ask.prompt().contains("You: Why no GC?\nYou: Why no GC?\nReply"), "failed turns are left out");
    m.receive(ask, reply("Ownership frees memory."));
    assert_eq!(m.layout().chat.last(), Some(&Turn::Ai("Ownership frees memory.".into())));
    let ask = m.step(vec![Cmd::Chat("Example?".into())]).pop().unwrap();
    assert!(ask.prompt().contains("AI: Ownership frees memory.\nYou: Example?\n"));
    m.receive(ask, reply("Box."));
    for q in ["Q1", "Q2"] {
        let ask = m.step(vec![Cmd::Chat(q.into())]).pop().unwrap();
        m.receive(ask, reply(&format!("A{}", &q[1..])));
    }
    let ask = m.step(vec![Cmd::Chat("Last?".into())]).pop().unwrap();
    assert!(
        ask.prompt().ends_with("A language.\nAI: Box.\nYou: Q1\nAI: A1\nYou: Q2\nAI: A2\nYou: Last?\nReply to the last message about \"Rust\", briefly, in Markdown. When it states facts, end with a \"Sources\" list of links you are sure exist. Reply with only the Markdown."),
        "the note stays and only the last 6 turns go: {}",
        ask.prompt()
    );
    assert_eq!(m.to_markdown(), "# Rust\n> A language.\n", "chat is not saved");

    m.step(vec![Cmd::Rename(m.current(), t("Go"))]);
    assert!(m.layout().chat.is_empty(), "a rename starts a new discussion");
    m.receive(ask, reply("About Rust."));
    assert!(m.layout().chat.is_empty(), "a reply for the old name is dropped");

    let ask = m.step(vec![Cmd::Reroll]).pop().unwrap();
    m.receive(ask, Ok(ghosts(vec![t("Goroutines")])));
    let keep = |m: &mut Map| {
        m.step(vec![Cmd::Keep(0)]);
        let kept = m.layout().nodes.last().unwrap().id;
        m.step(vec![Cmd::Select(kept)]);
    };
    keep(&mut m);
    let ask = m.step(vec![Cmd::Chat("What is it?".into())]).pop().unwrap();
    m.step(vec![Cmd::Delete]);
    m.receive(ask, reply("Late."));
    keep(&mut m);
    assert!(m.layout().chat.is_empty(), "a reply for a deleted node lands nowhere");
}
