use super::*;

#[test]
fn edit_a_note_in_the_inspector() {
    let (mut harness, path) = harness("note");
    start(&mut harness, "Rust");
    harness.get_by_label("Edit").click();
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    // The note box comes after the Discuss box, which is pinned under the scrolling part.
    harness.query_all_by_role(egui::accesskit::Role::MultilineTextInput).last().unwrap().type_text("Uses `cargo`.\n\n- fast");
    harness.get_by_label("Done").click();
    harness.run();
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Rust\n> Uses `cargo`.\n>\n> - fast\n");
    shot(&mut harness, "4-note");
}

#[test]
fn a_note_being_typed_is_saved_when_its_tab_or_the_window_closes() {
    let type_note = |harness: &mut Harness<App>, text: &str| {
        harness.get_by_label("Edit").click();
        harness.run();
        harness.query_all_by_role(egui::accesskit::Role::MultilineTextInput).last().unwrap().type_text(text);
        harness.run();
    };
    let (mut h, path) = harness("note-exit");
    start(&mut h, "Rust");
    type_note(&mut h, "Typed, never Done.");
    eframe::App::on_exit(h.state_mut());
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Rust\n> About it.Typed, never Done.\n");

    let (mut harness, path) = harness("note-close");
    start(&mut harness, "Rust");
    harness.state_mut().docs.push(Doc::open(temp("note-close-other")).unwrap());
    type_note(&mut harness, "Typed, then closed.");
    harness.get_all_by_label("×").next().unwrap().click();
    harness.run();
    assert_eq!(harness.state().docs.len(), 1);
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Rust\n> About it.Typed, then closed.\n");
}

#[test]
fn learn_more_adds_an_in_depth_note() {
    let (mut harness, path) = harness("learn");
    start(&mut harness, "Rust");
    harness.key_press(Key::L);
    wait_for(&mut harness, "How it works");
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Rust\n> About it.\n>\n> ## How it works\n> Deep **detail**.\n");
    shot(&mut harness, "6-learn");
}

#[test]
fn map_out_grows_a_brainstormed_node_into_a_subtree() {
    let (mut harness, path) = harness("grow");
    start(&mut harness, "Ideas");
    harness.get_by_label("suggestion: Alpha").click_accesskit();
    wait_for(&mut harness, "node: Alpha");
    // Alpha's note arriving re-numbers the panel's widgets, and a click sent by id across that would miss.
    wait_for(&mut harness, "About it.");
    harness.get_by_label("Map out").click();
    wait_for(&mut harness, "node: Ownership");
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "# Ideas\n> About it.\n- Alpha\n  > About it.\n  - Ownership\n    > Who frees memory.\n    - Borrowing\n  - Cargo\n",
        "the branches land under Alpha and its note stays"
    );
    shot(&mut harness, "19-map-out");
}

#[test]
fn examples_come_with_sources_and_dead_links_are_marked() {
    let (mut harness, path) = harness("examples");
    start(&mut harness, "Rust");
    assert!(shows(&harness, "From the model's own knowledge, no sources cited"), "a note without links says where it came from");
    assert!(shows(&harness, NO_WEB), "the fake backend cannot search the web");
    let links = &mut harness.state_mut().docs[0].view.lookup.links.got;
    links.insert("https://ok.example/box".into(), wiki::Link::Ok);
    links.insert("https://dead.example/rc".into(), wiki::Link::Dead("HTTP 404".into()));
    harness.key_press(Key::E);
    wait_for(&mut harness, "Sources: 2 links, 1 reachable, 1 dead");
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "# Rust\n> About it.\n>\n> ## Examples\n> - [Box](https://ok.example/box) owns its value.\n> - [Rc](https://dead.example/rc) shares it.\n"
    );
    assert!(shows(&harness, "Dead: https://dead.example/rc (HTTP 404)") && !shows(&harness, "From the model's own knowledge, no sources cited"));
    steps(&mut harness, 3);
    shot(&mut harness, "18-sources");
}

#[test]
fn a_wide_note_scrolls_inside_the_inspector_instead_of_widening_it() {
    let path = temp("wide");
    let long = "x".repeat(300);
    fs::write(&path, format!("# Rust\n> About it.\n>\n> ```rust\n> let {long} = 1;\n> ```\n> https://dead.example/{long}\n")).unwrap();
    let tab = Doc::open(path).unwrap();
    let mut harness = Harness::builder().with_size(Vec2::new(1000.0, 700.0)).build_eframe(move |_| App::new(ai::Config { models: vec![fake_ai()], footer: None, skills: Vec::new(), ghosts: None }, vec![tab], None));
    harness.state_mut().docs[0].view.lookup.links.got.insert(format!("https://dead.example/{long}"), wiki::Link::Dead("HTTP 404".into()));
    steps(&mut harness, 10);
    let left = harness.get_by_label("Topic").rect().left();
    assert!(left > 1000.0 - 400.0, "the inspector stays near its 340 px width, its left edge is at {left}");
}

#[test]
fn a_wrapped_code_reply_does_not_widen_the_topic_panel_on_a_scaled_screen() {
    let doc = Doc::open(temp("wide-chat")).unwrap();
    let mut harness = Harness::builder().with_size(Vec2::new(1000.0, 700.0)).with_pixels_per_point(2.0).build_eframe(move |_| App::new(ai::Config { models: vec![fake_ai()], footer: None, skills: Vec::new(), ghosts: None }, vec![doc], None));
    harness.state_mut().settings.zoom = 1.25;
    start(&mut harness, "Rust");
    let before = harness.get_by_label("Topic").rect().left();
    harness.get_by_role(egui::accesskit::Role::MultilineTextInput).click();
    harness.run();
    harness.get_by_role(egui::accesskit::Role::MultilineTextInput).type_text("Show wide code");
    harness.key_press(Key::Enter);
    wait_for(&mut harness, "Add to note");
    steps(&mut harness, 40);
    let left = harness.get_by_label("Topic").rect().left();
    assert!((left - before).abs() < 1.0, "the panel keeps its width, its left edge moved from {before} to {left}");
}

#[test]
fn learn_more_follows_the_writing_guides_that_are_on() {
    let path = temp("guides");
    let doc = Doc::open(path.clone()).unwrap();
    let skills = vec![ai::Skill { name: "Pirate".into(), text: "Write like a pirate.".into(), on: true }];
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1000.0, 700.0))
        .build_eframe(move |_| App::new(ai::Config { models: vec![fake_ai()], footer: None, skills, ghosts: None }, vec![doc], None));
    start(&mut harness, "Rust");
    // Ghost requests skip the guides, or the fake would not have answered with Alpha.
    harness.key_press(Key::L);
    wait_for(&mut harness, "guided");
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Rust\n> About it.\n>\n> Arr, **guided**.\n");
    harness.get_by_label("Aa").click();
    harness.run();
    shot(&mut harness, "12-writing-guides");
    harness.get_by_label("Pirate").click();
    harness.run();
    assert!(!harness.state().skills[0].on);
    harness.key_press(Key::Escape);
    harness.run();
    harness.key_press(Key::L);
    wait_for(&mut harness, "How it works");
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Rust\n> About it.\n>\n> Arr, **guided**.\n>\n> ## How it works\n> Deep **detail**.\n");
}

#[test]
fn discuss_a_node_and_add_the_reply_to_its_note() {
    let (mut harness, path) = harness("chat");
    start(&mut harness, "Rust");
    // The Discuss box is the only multi-line box while no note is edited. Typing needs it focused a frame first.
    harness.get_by_role(egui::accesskit::Role::MultilineTextInput).click();
    harness.run();
    harness.get_by_role(egui::accesskit::Role::MultilineTextInput).type_text("How do I share data?");
    harness.key_press_modifiers(Modifiers::SHIFT, Key::Enter);
    harness.run();
    assert_eq!(harness.state().docs[0].view.chat_draft, "How do I share data?\n", "Shift+Enter starts a new line");
    harness.key_press(Key::Backspace);
    harness.key_press(Key::Enter);
    steps(&mut harness, 6);
    assert!(harness.state().docs[0].view.chat_draft.is_empty(), "Enter sends");
    wait_for(&mut harness, "Add to note");
    assert!(shows(&harness, "How do I share data?"));
    steps(&mut harness, 3);
    shot(&mut harness, "10-chat");
    harness.get_by_label("Add to note").click();
    harness.run();
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Rust\n> About it.\n>\n> Try **Box**.\n");
    harness.get_by_label("Aa").click();
    harness.run();
    harness.get_by_label("Mono").click();
    harness.run();
    assert!(harness.state().settings.mono);
    shot(&mut harness, "11-note-text");
}

#[test]
fn the_topic_panel_links_a_node_to_wikipedia_and_youtube_and_shows_its_card() {
    let (mut harness, _) = harness("wiki");
    harness.state_mut().settings.wiki = false;
    start(&mut harness, "Rust");
    let thumb = "https://thumb.wikimedia.org/rust.png";
    let card_title = "Rust (programming language)";
    let mut png = std::io::Cursor::new(Vec::new());
    image::RgbImage::from_pixel(320, 180, image::Rgb([200, 120, 60])).write_to(&mut png, image::ImageFormat::Png).unwrap();
    egui_extras::install_image_loaders(&harness.ctx);
    let png = png.into_inner();
    let more = ["https://up.example/two.png", "https://up.example/three.png"];
    for uri in [thumb].iter().chain(&more) {
        harness.ctx.include_bytes(*uri, png.clone());
    }
    let card = wiki::Card {
        title: card_title.into(),
        extract: "Rust is a general-purpose programming language.".into(),
        url: "https://en.wikipedia.org/wiki/Rust_(programming_language)".into(),
        thumb: None,
    };
    let active = harness.state().active;
    harness.state_mut().docs[active].view.lookup.wiki.got.insert("Rust".into(), Ok(vec![card]));
    // The hit has no thumbnail, so the picture comes from the article's images or Commons.
    harness.state_mut().docs[active].view.lookup.images.got.insert(card_title.into(), [thumb].iter().chain(&more).map(|uri| uri.to_string()).collect());
    harness.state_mut().settings.wiki = true;
    harness.run();
    assert!(shows(&harness, "Wikipedia") && shows(&harness, "YouTube"));
    assert!(!shows(&harness, "Rust is a general-purpose programming language."), "the summary lives in the article tab, not the panel");
    let read = harness.get_by_label("Read").rect();
    for link in ["Wikipedia", "YouTube"] {
        let at = harness.get_by_label(link).rect();
        assert!((at.center().y - read.center().y).abs() < 4.0 && at.right() < read.left(), "{link} sits left of Read in the header");
    }
    let image = harness.get_by_role(egui::accesskit::Role::Image).rect();
    assert!(image.bottom() <= harness.get_by_label("Auto-suggest").rect().top(), "the pictures lead the panel");
    assert!(shows(&harness, "1 / 3"));
    harness.get_by_label("‹").click();
    harness.run();
    assert!(shows(&harness, "3 / 3"), "the slider wraps around");
    harness.get_by_label("›").click();
    harness.run();
    assert!(shows(&harness, "1 / 3"));
    shot(&mut harness, "17-wiki-card");

    harness.state_mut().article_text.got.insert(card_title.into(), Ok("Rust is fast.\n\n## History\n\nBegun in 2006.\n".into()));
    harness.get_by_label("Wikipedia").click();
    harness.run();
    assert!(shows(&harness, "W Rust (programming language)") && shows(&harness, "Open on Wikipedia") && shows(&harness, "Begun in 2006."));
    assert!(!shows(&harness, "node: Rust"), "the article replaces the map");
    shot(&mut harness, "18-wiki-article");
    let map = harness.state().docs[active].name();
    harness.get_by_label(&format!("‹ Back to {map}")).click();
    harness.run();
    assert!(shows(&harness, "node: Rust") && harness.state().articles.len() == 1, "Back shows the map and keeps the article tab");
    harness.get_by_label("W Rust (programming language)").click();
    harness.run();
    harness.key_press(Key::Escape);
    harness.run();
    assert!(shows(&harness, "node: Rust"), "Esc goes back too");
    harness.get_by_role(egui::accesskit::Role::Image).click();
    harness.run();
    assert!(shows(&harness, "Next ›"), "the picture opens the viewer");
    let big = harness.query_all_by_role(egui::accesskit::Role::Image).map(|n| n.rect().width()).fold(0.0, f32::max);
    assert!(big > 700.0, "the viewer's picture fills most of the 1000 px window, not the panel: {big}");
    harness.key_press(Key::ArrowRight);
    harness.run();
    assert!(shows(&harness, "2 / 3"), "the right arrow shows the next picture");
    assert_eq!(current(&harness), harness.state().docs[active].map().unwrap().layout().nodes[0].id, "the arrow did not walk the map");
    shot(&mut harness, "20-picture-viewer");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!shows(&harness, "Next ›") && shows(&harness, "node: Rust"), "Esc closes the viewer and stays on the node");
    harness.get_by_role(egui::accesskit::Role::Image).click();
    harness.run();
    harness.get_by_label(&format!("Read \"{card_title}\"")).click();
    harness.run();
    assert!(shows(&harness, "Begun in 2006."), "the viewer opens the article too");
    assert_eq!(harness.state().articles.len(), 1, "the same tab, not a second one");
    assert_eq!(harness.state().articles[0].thumb.as_deref(), Some(thumb), "the tab keeps the fallback picture");
    harness.get_by_label("×").click();
    harness.run();
    assert!(harness.state().articles.is_empty() && shows(&harness, "node: Rust"), "closing the article shows the map again");

    harness.get_by_label("Aa").click();
    harness.run();
    harness.get_by_label("Wikipedia card").click();
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.state().settings.wiki);
    assert!(harness.query_all_by_role(egui::accesskit::Role::Image).next().is_none(), "no picture with the card off");
    harness.get_by_label("suggestion: Alpha").click_accesskit();
    wait_for(&mut harness, "node: Alpha");
    assert!(shows(&harness, "YouTube"), "the links need no setting");
    assert!(!harness.state().docs[active].view.lookup.wiki.got.contains_key("Rust Alpha"), "nothing was sent to Wikipedia");

    harness.state_mut().settings.wiki = true;
    harness.run();
    assert!(harness.state().docs[active].view.lookup.wiki.got.contains_key("Rust Alpha"), "Alpha is looked up");
    assert!(shows(&harness, "1 / 3"), "Alpha has no pictures of its own, so it shows Rust's");
}

#[test]
fn a_note_shows_html_as_text_and_tries_its_pictures() {
    use super::widgets::without_tags;
    assert_eq!(without_tags("<details>\n<summary>More</summary>hidden <b>here</b></details>"), "More hidden here");
    let path = temp("html-note");
    fs::write(&path, "# Rust\n> Before.\n>\n> <details><summary>More</summary>hidden</details>\n>\n> ![a crab](https://example.com/crab.png)\n").unwrap();
    let doc = Doc::open(path).unwrap();
    let mut harness = Harness::builder().with_size(Vec2::new(1000.0, 700.0)).with_max_steps(8).build_eframe(move |cc| {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        cc.egui_ctx.add_bytes_loader(std::sync::Arc::new(wiki::WebImages::default()));
        App::new(ai::Config { models: vec![fake_ai()], footer: None, skills: Vec::new(), ghosts: None }, vec![doc], None)
    });
    steps(&mut harness, 10);
    assert!(shows(&harness, "More hidden") && !harness.query_all_by_label_contains("<details>").next().is_some(), "the HTML block reads as text");
}

#[test]
fn latex_in_a_note_draws_as_math() {
    let path = temp("math-note");
    fs::write(&path, "# Rust\n> Energy is $E = mc^2$.\n>\n> $$\\sum_{i=1}^{n} \\frac{1}{i^2}$$\n>\n> Broken: $\\frac{1}{$ here.\n").unwrap();
    let doc = Doc::open(path).unwrap();
    let mut harness = Harness::builder().with_size(Vec2::new(1000.0, 700.0)).with_max_steps(8).build_eframe(move |cc| {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        App::new(ai::Config { models: vec![fake_ai()], footer: None, skills: Vec::new(), ghosts: None }, vec![doc], None)
    });
    steps(&mut harness, 10);
    assert_eq!(harness.query_all_by_role(egui::accesskit::Role::Image).count(), 2, "the inline and the block formula are pictures");
    assert!(!harness.query_all_by_label_contains("mc^2").next().is_some(), "no raw TeX for a formula that parses");
    assert!(shows(&harness, "\\frac{1}{"), "a formula that does not parse shows its source");
    shot(&mut harness, "26-math");
}
