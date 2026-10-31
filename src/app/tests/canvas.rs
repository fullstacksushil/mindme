use super::*;

#[test]
fn drag_a_node_and_the_graph_settles() {
    let (mut harness, _) = harness("drag");
    start(&mut harness, "Rust");
    let root = Body::Node(current(&harness));
    for _ in 0..200 {
        harness.step();
    }
    let before = harness.state().docs[0].view.graph.pos(&root).unwrap();
    // Accesskit reports canvas coordinates inside the Scene's layer, so map the node to the screen first.
    let from = harness.state().docs[0].view.to_screen(before);
    harness.drag_at(from);
    harness.step();
    for i in 1..=10 {
        harness.hover_at(from + Vec2::new(20.0 * i as f32, 0.0));
        harness.step();
    }
    harness.drop_at(from + Vec2::new(200.0, 0.0));
    harness.step();
    let after = harness.state().docs[0].view.graph.pos(&root).unwrap();
    assert!(after.x - before.x > 50.0, "the node followed the pointer: {before:?} -> {after:?}");
    let mut frames = 0;
    while !harness.state().docs[0].view.graph.settled() && frames < 300 {
        harness.step();
        frames += 1;
    }
    assert!(frames <= 120, "settled within 2 seconds at 60 fps, took {frames} frames");
}

#[test]
fn screenshots() {
    let (mut harness, _) = harness("shots");
    harness.run();
    shot(&mut harness, "0-start");
    start(&mut harness, "Rust");
    for _ in 0..150 {
        harness.step();
    }
    shot(&mut harness, "1-ghosts");
    for _ in 0..3 {
        let first = harness.query_all_by_label_contains("suggestion: ").next().unwrap().accesskit_node().label().unwrap();
        harness.get_by_label(&first).click_accesskit();
        for _ in 0..30 {
            harness.step();
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    harness.key_press(Key::Escape);
    harness.key_press(Key::Escape);
    for _ in 0..200 {
        harness.step();
    }
    harness.key_press(Key::F);
    for _ in 0..5 {
        harness.step();
    }
    shot(&mut harness, "2-graph");
}

#[test]
fn outline_selects_a_node() {
    let (mut harness, _) = harness("outline");
    start(&mut harness, "Rust");
    harness.get_by_label("suggestion: Alpha").click_accesskit();
    wait_for(&mut harness, "suggestion: Beta");
    let alpha = current(&harness);
    // The first "Rust" is the outline entry. The inspector's path link to the root comes after it.
    harness.query_all_by_label("Rust").next().unwrap().click();
    for _ in 0..3 {
        harness.step();
    }
    assert_ne!(current(&harness), alpha, "the outline moved the current node to the root");
    assert!(harness.query_by_label("node: Rust").is_some(), "the canvas still shows the root");
}

#[test]
fn without_auto_suggest_tab_builds_a_map_by_hand() {
    let (mut harness, path) = harness("by-hand");
    harness.run();
    harness.get_by_label("Auto-suggest").click();
    harness.run();
    shot(&mut harness, "13-start-manual");
    harness.get_by_role(egui::accesskit::Role::TextInput).type_text("Trip");
    harness.key_press(Key::Enter);
    harness.run();
    for branch in ["Packing", "Clothes"] {
        harness.key_press(Key::Tab);
        type_branch(&mut harness, branch);
    }
    let clothes = current(&harness);
    harness.key_press(Key::Tab);
    harness.run();
    assert!(adding_box(&harness), "Tab opened the new branch box");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!adding_box(&harness) && current(&harness) == clothes, "Esc closed the box without going to the parent");
    harness.key_press(Key::Tab);
    type_branch(&mut harness, "");
    assert!(!adding_box(&harness), "an empty Enter closes it too");
    harness.key_press(Key::Escape);
    harness.run();
    harness.key_press(Key::Escape);
    harness.run();
    harness.get_by_label("+ Add").click();
    type_branch(&mut harness, "Budget");
    steps(&mut harness, 30);
    assert_eq!(doc(&harness).in_flight, 0, "never asked the AI");
    assert!(harness.query_all_by_label_contains("suggestion: ").next().is_none());
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Trip\n- Packing\n  - Clothes\n- Budget\n");
    steps(&mut harness, 150);
    harness.key_press(Key::F);
    steps(&mut harness, 5);
    shot(&mut harness, "14-by-hand");

    harness.key_press(Key::M);
    wait_for(&mut harness, "suggestion: Alpha");
}

#[test]
fn backslash_shows_only_the_graph_and_r_reads_the_note() {
    let (mut harness, _) = harness("modes");
    start(&mut harness, "Rust");
    harness.get_by_label("suggestion: Alpha").click_accesskit();
    wait_for(&mut harness, "About it.");
    let alpha = current(&harness);
    harness.get_by_label("«").click();
    harness.run();
    harness.key_press(Key::Backslash);
    harness.run();
    assert!(!shows(&harness, "Topic") && shows(&harness, "Topic «") && shows(&harness, "» Outline"));
    steps(&mut harness, 100);
    shot(&mut harness, "15-graph-only");
    harness.key_press(Key::Tab);
    harness.run();
    shot(&mut harness, "15-add-floating");
    type_branch(&mut harness, "Gamma");
    assert!(shows(&harness, "node: Gamma"), "with the inspector hidden, the new branch box floats over the canvas");
    harness.key_press(Key::Escape);
    steps(&mut harness, 3); // The camera glides in wall time, so `run` can time out on a busy machine.
    harness.key_press(Key::Backslash);
    harness.run();
    assert!(shows(&harness, "Topic") && shows(&harness, "» Outline"), "the outline stays hidden as it was");

    harness.key_press(Key::R);
    harness.run();
    assert!(!shows(&harness, "node: Alpha"), "no canvas behind the reading column");
    let note = harness.query_all_by_label("About it.").next().unwrap().rect();
    let column = 680.0;
    assert!((note.left() - (500.0 - column / 2.0)).abs() < 20.0, "a {column} wide column centered in the 1000 wide window: {note:?}");
    shot(&mut harness, "16-reading");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(shows(&harness, "node: Alpha"));
    assert_eq!(current(&harness), alpha, "Esc left reading mode without going to the parent");
    harness.get_by_label("Read").click();
    harness.run();
    assert!(!shows(&harness, "node: Alpha"), "the inspector's Read button opens reading mode too");
}

#[test]
fn graph_settings_open_from_the_canvas_and_survive_a_restart() {
    let (mut harness, _) = harness("settings");
    start(&mut harness, "Rust");
    harness.get_by_label("Graph").click();
    harness.run();
    assert!(shows(&harness, "Repel force") && shows(&harness, "Link distance"));
    shot(&mut harness, "9-settings");

    let file = temp("settings-file");
    let mut settings = GraphSettings { text: 1.5, distance: 300.0, mono: true, wiki: false, light: true, theme: "gruvbox".into(), zoom: 1.25, text_font: "Inter".into(), ..GraphSettings::default() };
    settings.forces.repel = 4.0;
    settings.save(&file).unwrap();
    assert_eq!(GraphSettings::load(&file), settings);
    assert_eq!(GraphSettings::load(&temp("no-such-file")), GraphSettings::default());
}

/// A mapped-out map the size of a real one, fully open, for a person to check for overlaps in the screenshot.
#[test]
fn a_big_graph_spreads_out() {
    let path = temp("big");
    let mut md = String::from("# pico-8\n");
    for a in 0..8 {
        md += &format!("- Area number {a}\n");
        for b in 0..5 {
            md += &format!("  - Topic {a}.{b} with a long name\n");
            for c in 0..3 {
                md += &format!("    - Detail {a}.{b}.{c}\n");
            }
        }
    }
    fs::write(&path, md).unwrap();
    let doc = Doc::open(path).unwrap();
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1400.0, 1000.0))
        .build_eframe(move |_| App::new(ai::Config { models: vec![fake_ai()], footer: None, skills: Vec::new(), ghosts: None }, vec![doc], None));
    harness.run();
    harness.get_by_label("Show all").click();
    let mut frames = 0;
    while (frames < 5 || !harness.state().docs[0].view.graph.settled()) && frames < 600 {
        harness.step();
        frames += 1;
    }
    assert!(frames < 600, "161 nodes settle");
    harness.key_press(Key::F);
    steps(&mut harness, 3);
    shot(&mut harness, "8-big");
}
#[test]
fn ctrl_z_undoes_a_change_and_ctrl_shift_z_redoes_it() {
    let (mut harness, path) = harness("undo");
    harness.run();
    harness.get_by_label("Auto-suggest").click();
    harness.run();
    harness.get_by_role(egui::accesskit::Role::TextInput).type_text("Trip");
    harness.key_press(Key::Enter);
    harness.run();
    harness.key_press(Key::Tab);
    type_branch(&mut harness, "Packing");
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Trip\n- Packing\n");
    harness.key_press_modifiers(Modifiers::CTRL, Key::Z);
    harness.run();
    assert!(!shows(&harness, "node: Packing"), "Ctrl+Z took the branch back");
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Trip\n", "the file follows");
    harness.key_press_modifiers(Modifiers::CTRL | Modifiers::SHIFT, Key::Z);
    harness.run();
    assert!(shows(&harness, "node: Packing"), "Ctrl+Shift+Z brought it back");
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Trip\n- Packing\n");

    // The name box is the only one-line box left, and a filled box reports no placeholder.
    let name_box = |h: &Harness<App>| h.get_by_role(egui::accesskit::Role::TextInput).accesskit_node().value().unwrap_or_default();
    assert_eq!(name_box(&harness), "Packing");
    harness.get_by_role(egui::accesskit::Role::TextInput).click();
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    type_branch(&mut harness, "Luggage");
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Trip\n- Luggage\n");
    harness.key_press_modifiers(Modifiers::CTRL, Key::Z);
    harness.run();
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Trip\n- Packing\n");
    assert_eq!(name_box(&harness), "Packing", "the name box shows the restored name");
    shot(&mut harness, "18-undo");
}


#[test]
fn plus_zooms_in_even_on_a_small_fresh_map() {
    let (mut harness, _) = harness("zoom");
    start(&mut harness, "Rust");
    steps(&mut harness, 100);
    let zoom = |h: &Harness<App>| {
        let view = &h.state().docs[0].view;
        (view.canvas.unwrap().size() / view.camera.unwrap().size()).min_elem()
    };
    let fitted = zoom(&harness);
    assert!(fitted <= 1.5 + 1e-3, "a small map opens no closer than 1.5×: {fitted}");
    // The canvas corner's + and -, after the tab strip's + menu.
    for _ in 0..2 {
        harness.query_all_by_label("+").last().unwrap().click();
        steps(&mut harness, 5);
    }
    assert!(zoom(&harness) > fitted * 1.4, "+ zooms past the fitted size: {} from {fitted}", zoom(&harness));
    harness.query_all_by_label("-").last().unwrap().click();
    steps(&mut harness, 5);
    assert!(zoom(&harness) < fitted * 1.4, "- zooms back out");
}
