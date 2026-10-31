use super::*;

#[test]
fn type_an_idea_and_keep_a_ghost() {
    let (mut harness, path) = harness("keep");
    start(&mut harness, "Rust");
    // Scene transforms the layer, so a pointer click at the reported rect misses. The accesskit click goes to the widget.
    harness.get_by_label("suggestion: Alpha").click_accesskit();
    wait_for(&mut harness, "node: Alpha");
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Rust\n> About it.\n- Alpha\n");
}

#[test]
fn a_mapped_out_topic_opens_one_level_at_a_time_and_asks_the_ai_only_on_command() {
    let (mut harness, path) = harness("map-out");
    harness.run();
    harness.get_by_role(egui::accesskit::Role::TextInput).type_text("Rust");
    harness.get_by_label("Map out").click();
    wait_for(&mut harness, "node: Rust");
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "# Rust\n> A **language**.\n- Ownership\n  > Who frees memory.\n  - Borrowing\n- Cargo\n"
    );
    assert!(!shows(&harness, "node: Ownership"), "starts at the root alone");

    harness.get_by_label("node: Rust").click_accesskit();
    steps(&mut harness, 3);
    assert!(shows(&harness, "node: Ownership") && shows(&harness, "node: Cargo") && !shows(&harness, "node: Borrowing"));
    harness.key_press(Key::ArrowDown);
    harness.key_press(Key::ArrowRight);
    steps(&mut harness, 3);
    assert!(shows(&harness, "node: Borrowing"), "→ opens the current node one level");
    harness.key_press(Key::ArrowLeft);
    // Long enough for the outline's fold animation to finish.
    steps(&mut harness, 30);
    assert!(!shows(&harness, "node: Borrowing"), "← folds it away again");
    assert_eq!(harness.query_all_by_label("Borrowing").count(), 1, "only the inspector lists it; the outline folded");

    steps(&mut harness, 30);
    assert_eq!(doc(&harness).in_flight, 0, "moving around never asked the AI");
    assert!(harness.query_all_by_label_contains("suggestion: ").next().is_none());
    harness.key_press(Key::M);
    wait_for(&mut harness, "suggestion: Alpha");

    harness.get_by_label("Show all").click();
    steps(&mut harness, 3);
    assert!(shows(&harness, "node: Borrowing"));
    steps(&mut harness, 200);
    harness.key_press(Key::F);
    harness.step();
    shot(&mut harness, "3-mapped-out");
    harness.get_by_label("Collapse all").click();
    steps(&mut harness, 3);
    assert!(shows(&harness, "node: Rust") && !shows(&harness, "node: Ownership"));
}

#[test]
fn tabs_keep_separate_maps_and_files() {
    let (mut harness, first) = harness("tab-a");
    start(&mut harness, "Rust");
    let second = temp("tab-b");
    harness.key_press_modifiers(Modifiers::CTRL, Key::T);
    harness.run();
    // The app draws the tab strip first, so its path box is the first text box.
    harness.query_all_by_role(egui::accesskit::Role::TextInput).next().unwrap().type_text(second.to_str().unwrap());
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(harness.state().active, 1, "the new file opened in a second tab");
    start(&mut harness, "Go");
    harness.get_by_label(first.file_name().unwrap().to_str().unwrap()).click();
    harness.run();
    assert_eq!(harness.state().active, 0);
    assert!(harness.query_by_label("node: Rust").is_some() && harness.query_by_label("node: Go").is_none());
    assert_eq!(fs::read_to_string(&first).unwrap(), "# Rust\n> About it.\n");
    assert_eq!(fs::read_to_string(&second).unwrap(), "# Go\n> About it.\n");
}

#[test]
fn grill_me_interviews_first_and_the_brief_goes_with_the_map() {
    let (mut harness, path) = harness("grill");
    harness.run();
    harness.get_by_label("Grill me").click();
    harness.run();
    harness.get_by_role(egui::accesskit::Role::TextInput).type_text("Rust");
    harness.key_press(Key::Enter);
    wait_for(&mut harness, "What is it for?");
    assert!(doc(&harness).map().is_none(), "no map until the interview ends");
    assert!(shows(&harness, "1 of up to 6") && shows(&harness, "Use recommended"));
    shot(&mut harness, "20-grill");
    harness.get_by_role(egui::accesskit::Role::MultilineTextInput).type_text("Writing a game engine");
    harness.key_press(Key::Enter);
    wait_for(&mut harness, "suggestion: Alpha");
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Rust\n<!-- brief\nQ: What is it for?\nA: Writing a game engine\n-->\n> About it.\n");
    harness.get_by_label("Interview").click();
    harness.run();
    assert!(shows(&harness, "Q: What is it for?\nA: Writing a game engine"), "the root shows what every request carries");
}

#[test]
fn grill_me_before_mapping_out_and_start_now_skips_the_rest() {
    let (mut harness, path) = harness("grill-map-out");
    harness.run();
    harness.get_by_label("Grill me").click();
    harness.run();
    harness.get_by_role(egui::accesskit::Role::TextInput).type_text("Rust");
    harness.get_by_label("Map out").click();
    wait_for(&mut harness, "What is it for?");
    harness.get_by_label("Use recommended").click();
    wait_for(&mut harness, "Branches (2)");
    assert!(fs::read_to_string(&path).unwrap().starts_with("# Rust\n<!-- brief\nQ: What is it for?\nA: Systems work\n-->\n> A **language**.\n- Ownership\n"));

    let (mut skip, path) = super::tests::harness("grill-skip");
    skip.run();
    skip.get_by_label("Grill me").click();
    skip.run();
    skip.get_by_role(egui::accesskit::Role::TextInput).type_text("Rust");
    skip.key_press(Key::Enter);
    wait_for(&mut skip, "What is it for?");
    skip.get_by_label("Start the brainstorm now").click();
    wait_for(&mut skip, "suggestion: Alpha");
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Rust\n> About it.\n", "no answers, no brief");
}

#[test]
fn an_opened_map_with_branches_waits_to_be_asked() {
    let (grown, bare) = (temp("grown"), temp("bare"));
    fs::write(&grown, "# Rust\n- Ownership\n").unwrap();
    fs::write(&bare, "# Rust\n").unwrap();
    assert!(!Doc::open(grown).unwrap().map().unwrap().layout().auto, "a map with branches opens with Auto-suggest off");
    assert!(Doc::open(bare).unwrap().map().unwrap().layout().auto, "a bare title still brainstorms");
}

#[test]
fn the_start_screen_warns_only_when_the_model_cannot_search_the_web() {
    let (mut harness, _) = harness("no-web");
    harness.run();
    assert!(shows(&harness, NO_WEB));
    let doc = Doc::open(temp("web")).unwrap();
    // `sh -c` takes the extra word as `$0`, so the allowed-tools name changes nothing it runs.
    let model = ai::Model::new(ai::Backend::Cli { command: "sh".into(), args: vec!["-c".into(), "cat >/dev/null".into(), "WebSearch".into()] });
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1000.0, 700.0))
        .build_eframe(move |_| App::new(ai::Config { models: vec![model], footer: None, skills: Vec::new(), ghosts: None }, vec![doc], None));
    harness.run();
    assert!(!shows(&harness, NO_WEB));
}

#[test]
fn recent_files_survive_a_restart_and_open_into_the_blank_tab() {
    let (first, second, dir) = (temp("recent-a"), temp("recent-b"), temp("recent-dir"));
    let list = dir.join("recent");
    fs::write(&first, "# Alpha map\n").unwrap();
    let app = |path: &PathBuf, list: &PathBuf| {
        let (doc, list) = (Doc::open(path.clone()).unwrap(), list.clone());
        Harness::builder().with_size(Vec2::new(1000.0, 700.0)).build_eframe(move |_| {
            App::new(ai::Config { models: vec![fake_ai()], footer: None, skills: Vec::new(), ghosts: None }, vec![doc], Some(list))
        })
    };
    let mut harness = app(&first, &dir);
    harness.run();
    assert_eq!(recent::load(&list), std::slice::from_ref(&first), "the file opened at launch is remembered");
    drop(harness);

    let mut harness = app(&second, &dir);
    harness.run();
    shot(&mut harness, "7-start-recent");
    let name = first.file_name().unwrap().to_str().unwrap();
    // The recent card is a button named for the file, with "edited … ago" under it.
    harness.query_all_by_label(name).find(|n| n.accesskit_node().role() == egui::accesskit::Role::Button).unwrap().click();
    steps(&mut harness, 3); // The camera glides in wall time, so `run` can time out on a busy machine.
    assert_eq!(harness.state().docs.len(), 1, "the blank tab gave its place to the recent file");
    assert!(shows(&harness, "node: Alpha map"));
    assert_eq!(recent::load(&list), [first.clone(), second.clone()], "newest first");
}

#[test]
fn graph_settings_that_failed_to_save_are_saved_once_they_can_be() {
    // A file where the config folder should be, so the folder cannot be made.
    let dir = temp("blocked-config");
    fs::write(&dir, "").unwrap();
    let doc = Doc::open(temp("blocked-map")).unwrap();
    let mut harness = Harness::builder().build_eframe(|_| App::new(ai::Config { models: vec![fake_ai()], footer: None, skills: Vec::new(), ghosts: None }, vec![doc], Some(dir.clone())));
    harness.state_mut().settings.wiki = !harness.state().settings.wiki;
    harness.run();
    assert_eq!(harness.state().open_error.as_deref(), Some("could not save the graph settings"));
    fs::remove_file(&dir).unwrap();
    harness.run();
    assert_eq!(GraphSettings::load(&dir.join("graph.json")).wiki, harness.state().settings.wiki, "the next frame retried");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_plus_menu_copies_the_map_as_markdown_or_json() {
    let (mut harness, _) = harness("export");
    start(&mut harness, "Rust");
    let map = doc(&harness).map().unwrap();
    let (md, json) = (map.to_markdown(), map.to_json());
    for (item, want) in [("Copy map as Markdown", md), ("Copy map as JSON", json)] {
        // The tab strip's + comes before the canvas zoom button.
        harness.query_all_by_label("+").next().unwrap().click();
        steps(&mut harness, 2);
        assert!(shows(&harness, "Export…"));
        shot(&mut harness, "26-plus-menu");
        harness.get_by_label(item).click();
        harness.step();
        assert_eq!(copied(&harness), Some(want), "{item}");
        steps(&mut harness, 2);
    }
}

#[test]
fn a_template_card_starts_a_map_with_its_branches() {
    let (mut harness, path) = harness("template");
    harness.run();
    assert!(shows(&harness, "Templates") && shows(&harness, "The help map"));
    shot(&mut harness, "0-start");
    harness.query_all_by_label("SWOT").find(|n| n.accesskit_node().role() == egui::accesskit::Role::Button).unwrap().click();
    harness.run();
    assert!(harness.query_all_by_role(egui::accesskit::Role::TextInput).any(|n| n.accesskit_node().placeholder() == Some("What is the SWOT about?")), "no topic yet, so the card asks for one");
    harness.get_by_role(egui::accesskit::Role::TextInput).type_text("Mind-Me");
    harness.key_press(Key::Enter);
    wait_for(&mut harness, "node: Mind-Me");
    assert!(fs::read_to_string(&path).unwrap().starts_with("# Mind-Me\n- Strengths\n  > What does it do well, from the inside?\n- Weaknesses\n"), "the topic is the root and the template's branches follow");
}

#[test]
fn with_no_model_the_app_opens_on_setup_and_saves_the_choice() {
    let dir = temp("setup-dir");
    let doc = Doc::open(temp("setup")).unwrap();
    let config_dir = dir.clone();
    let mut harness = Harness::builder().with_size(Vec2::new(1000.0, 800.0)).with_max_steps(8).build_eframe(move |_| {
        App::new(ai::Config { models: Vec::new(), footer: None, skills: Vec::new(), ghosts: None }, vec![doc], Some(config_dir))
    });
    harness.run();
    assert!(shows(&harness, "Welcome to Mind-Me") && !shows(&harness, "Cancel"), "nothing else works yet, so there is no way back");
    harness.get_by_label("OpenAI Codex").click();
    harness.run();
    assert!(shows(&harness, "The codex command is not on your PATH. Install Codex and run codex login first."), "tests never look on PATH");
    harness.get_by_label("Another command").click();
    harness.run();
    harness.get_by_role(egui::accesskit::Role::TextInput).click();
    harness.run();
    harness.get_by_role(egui::accesskit::Role::TextInput).type_text("echo OK");
    harness.run();
    harness.get_by_label("Test").click();
    wait_for(&mut harness, "It answered.");
    shot(&mut harness, "28-setup");
    harness.get_by_label("Save").click();
    harness.run();
    assert_eq!(harness.state().models.len(), 1);
    assert!(shows(&harness, "Templates"), "the start screen follows");
    let saved: serde_json::Value = serde_json::from_str(&fs::read_to_string(dir.join("config.json")).unwrap()).unwrap();
    assert_eq!(saved["models"], serde_json::json!([{"name": "echo", "cmd": "echo OK"}]));

    harness.key_press_modifiers(Modifiers::CTRL, Key::Comma);
    harness.run();
    harness.get_by_label("Models").click();
    steps(&mut harness, 3);
    shot(&mut harness, "29-models");
    harness.get_by_label("Remove").click();
    harness.run();
    assert!(shows(&harness, "Remove \"echo\" from config.json?"));
    harness.query_all_by_label("Remove").last().unwrap().click();
    harness.run();
    assert!(harness.state().models.is_empty() && shows(&harness, "Welcome to Mind-Me"), "removing the last model goes back to setup");
}
