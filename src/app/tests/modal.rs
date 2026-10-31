use super::*;

#[test]
fn stop_kills_a_waiting_call_and_offers_a_retry() {
    let (mut harness, _) = harness("stop");
    harness.run();
    harness.get_by_role(egui::accesskit::Role::TextInput).type_text("Slow");
    harness.key_press(Key::Enter);
    wait_for(&mut harness, "thinking…");
    assert_eq!(doc(&harness).in_flight, 1);
    shot(&mut harness, "21-stop");
    harness.get_by_label("Stop").click();
    wait_for(&mut harness, "suggestion: retry: stopped");
    for _ in 0..100 {
        if doc(&harness).in_flight == 0 {
            break;
        }
        harness.step();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(doc(&harness).in_flight, 0, "the hanging command was killed, not left running for 30 s");
}

#[test]
fn delete_and_more_ideas_ask_first() {
    let (mut harness, path) = harness("confirm");
    start(&mut harness, "Rust");
    harness.get_by_label("suggestion: Alpha").click_accesskit();
    wait_for(&mut harness, "suggestion: Beta");
    harness.key_press(Key::Delete);
    harness.run();
    assert!(shows(&harness, "Delete \"Alpha\"?"));
    shot(&mut harness, "22-confirm-delete");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!shows(&harness, "Delete \"Alpha\"?") && shows(&harness, "node: Alpha"), "Esc cancels and stays on the node");
    harness.key_press(Key::Delete);
    harness.run();
    harness.get_by_label("Cancel").click();
    harness.run();
    assert!(shows(&harness, "node: Alpha"), "Cancel keeps it");
    harness.key_press(Key::Delete);
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert!(!shows(&harness, "node: Alpha"), "yes deletes it");
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Rust\n> About it.\n");

    harness.get_by_label("suggestion: Alpha").click_accesskit();
    wait_for(&mut harness, "node: Alpha");
    harness.key_press(Key::Tab);
    type_branch(&mut harness, "Child");
    wait_for(&mut harness, "node: Child");
    harness.get_by_label("node: Alpha").click_accesskit();
    harness.run();
    harness.get_by_label("Delete").click();
    harness.run();
    assert!(shows(&harness, "Delete \"Alpha\" and its 1 branch?"), "a node with branches can be deleted too");
    harness.key_press(Key::Enter);
    harness.run();
    assert!(!shows(&harness, "node: Alpha") && !shows(&harness, "node: Child"), "its branch went with it");

    let asked = doc(&harness).usage.input;
    harness.key_press(Key::M);
    harness.run();
    assert!(shows(&harness, "Replace the suggestions for \"Rust\" with new ones?"));
    harness.key_press(Key::Escape);
    steps(&mut harness, 3);
    assert_eq!(doc(&harness).usage.input, asked, "a cancelled More ideas never asked");
    assert!(shows(&harness, "suggestion: Alpha"), "and kept the suggestions");
}

#[test]
fn settings_open_as_a_tab() {
    let (mut harness, _) = harness("settings");
    start(&mut harness, "Rust");
    harness.key_press_modifiers(Modifiers::CTRL, Key::Comma);
    harness.run();
    assert!(shows(&harness, "Text font") && !shows(&harness, "Center force") && !shows(&harness, "node: Rust"), "Appearance opens first, one tab at a time");
    shot(&mut harness, "23-settings");
    harness.get_by_label("Light").click();
    harness.run();
    assert!(harness.state().settings.light);
    harness.get_by_label("Theme").click();
    harness.run();
    harness.get_by_label("Gruvbox").click();
    steps(&mut harness, 3);
    let gruvbox = theme::themes(None).into_iter().find(|t| t.name == "gruvbox").unwrap();
    assert_eq!((harness.state().settings.theme.as_str(), harness.state().applied), ("gruvbox", gruvbox.light), "a click applies the theme, in the picked mode");
    shot(&mut harness, "24-settings-gruvbox-light");
    harness.get_by_label("Theme").click();
    harness.run();
    harness.get_by_label("Nord  · dark only").click();
    steps(&mut harness, 3);
    assert!(harness.get_by_label("Dark").accesskit_node().is_disabled(), "Nord has one mode to pick");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Equals);
    steps(&mut harness, 3);
    assert!(harness.state().settings.zoom > 1.0, "Ctrl+= zooms the whole UI and is saved: {}", harness.state().settings.zoom);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Num0);
    steps(&mut harness, 3);
    assert_eq!(harness.state().settings.zoom, 1.0);
    harness.get_by_label("Graph").click();
    harness.run();
    assert!(shows(&harness, "Center force") && shows(&harness, "Rearrange"));
    harness.state_mut().settings.distance = 400.0;
    harness.get_by_label("Reset this tab").click();
    harness.run();
    assert_eq!((harness.state().settings.distance, harness.state().settings.theme.as_str()), (150.0, "nord"), "resetting the graph leaves the theme alone");
    harness.get_by_label("Footer").click();
    harness.run();
    assert!(shows(&harness, "Cost (approx.)"));
    let name = doc(&harness).name();
    harness.get_by_label(&name).click();
    harness.run();
    assert!(shows(&harness, "node: Rust") && harness.state().settings_open, "the map shows and the Settings tab stays");
    // The tab comes before the footer's Settings button.
    harness.query_all_by_label("Settings").next().unwrap().click();
    harness.run();
    assert!(shows(&harness, "Appearance"));
    harness.get_by_label("×").click();
    harness.run();
    assert!(shows(&harness, "node: Rust") && !harness.state().settings_open);
}
