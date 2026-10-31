use super::*;

#[test]
fn the_footer_toggles_between_the_theme_s_dark_and_light_palettes() {
    let (mut harness, _) = harness("theme");
    start(&mut harness, "Rust");
    harness.state_mut().docs[0].save_error = Some("could not save: disk full".into());
    steps(&mut harness, 150);
    assert_eq!(harness.ctx.theme(), egui::Theme::Dark, "Mind-Me's dark theme by default, whatever the system theme");
    shot(&mut harness, "13-dark");
    harness.get_by_label("☀").click();
    steps(&mut harness, 3);
    assert_eq!(harness.ctx.theme(), egui::Theme::Light);
    assert!(harness.state().settings.light, "the choice is a graph setting, so it is saved");
    assert_eq!(harness.state().applied, Some(mindme(true)), "the same theme's light palette");
    harness.hover_at(Pos2::new(300.0, 600.0));
    harness.step();
    shot(&mut harness, "14-light");
    harness.key_press(Key::L);
    wait_for(&mut harness, "How it works");
    harness.get_by_label("Aa").click();
    harness.run();
    shot(&mut harness, "15-light-note");
    harness.key_press(Key::Escape);
    harness.run();
    harness.get_by_label("🌙").click();
    steps(&mut harness, 3);
    assert_eq!(harness.ctx.theme(), egui::Theme::Dark);

    harness.state_mut().settings.theme = "catppuccin".into();
    steps(&mut harness, 3);
    harness.get_by_label("☀").click();
    steps(&mut harness, 3);
    let latte = theme::themes(None).into_iter().find(|t| t.name == "catppuccin").unwrap().light;
    assert_eq!(harness.state().applied, latte, "Catppuccin's light palette is Latte");
    harness.state_mut().settings.theme = "nord".into();
    steps(&mut harness, 3);
    assert!(harness.get_by_label("☀").accesskit_node().is_disabled(), "Nord has no light palette, so there is nothing to toggle to");
    assert_eq!(harness.ctx.theme(), egui::Theme::Dark, "Nord stays dark even with light saved");
}

#[test]
fn a_wait_shows_how_long_it_has_been() {
    let node = Map::new(Text::new("Rust").unwrap()).layout().current;
    let shown = std::rc::Rc::new(std::cell::Cell::new(true));
    let on = shown.clone();
    let mut harness = Harness::builder().with_size(Vec2::new(300.0, 60.0)).build_ui(move |ui| {
        if on.get() {
            waiting(ui, (node, "learn"), "learning more…");
        }
    });
    let texts = |h: &Harness| -> Vec<String> { h.query_all(egui_kittest::kittest::by()).filter_map(|n| n.accesskit_node().value()).collect() };
    let now = |h: &Harness| h.ctx.input(|i| i.time);
    harness.step();
    let start = now(&harness);
    while now(&harness) - start < 2.5 {
        harness.step();
    }
    harness.step();
    assert_eq!(texts(&harness)[0], "learning more…");
    let secs: u64 = texts(&harness).last().unwrap().trim_end_matches('s').parse().unwrap();
    assert!(secs >= 2, "counts up while it shows, at {secs}s");
    shown.set(false);
    let hidden = now(&harness);
    while now(&harness) - hidden < 1.5 {
        harness.step();
    }
    shown.set(true);
    harness.step();
    assert_eq!(texts(&harness).last().unwrap(), "0s", "the next wait starts from zero");
}

#[test]
fn the_footer_adds_up_each_tabs_tokens_and_cost() {
    let (mut harness, _) = harness("usage");
    start(&mut harness, "Rust");
    assert_eq!(doc(&harness).usage, ai::Usage { input: 2000, output: 20, cached: 1990, cost: 0.0125, model: None });
    harness.run();
    for label in ["in 2.0k", "out 20", "total 2.0k", "cached 2.0k", "~$0.0125"] {
        assert!(shows(&harness, label), "footer shows {label}");
    }
    harness.get_by_label("…").click();
    harness.run();
    harness.get_by_label("Tokens in").click();
    harness.run();
    assert!(!shows(&harness, "in 2.0k"), "the menu hides a figure");
    harness.get_by_label("Keys").click();
    harness.run();
    assert!(shows(&harness, "Next tab"), "Keys opens the shortcut list on click");
    shot(&mut harness, "5-footer");
}

#[test]
fn the_footer_switches_the_tabs_model_after_the_start_screen() {
    let path = temp("models");
    fs::write(&path, "# Rust\n").unwrap();
    let tab = Doc::open(path).unwrap();
    let (mut first, mut other) = (fake_ai(), fake_ai());
    (first.name, other.name) = ("First".into(), "Other".into());
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1000.0, 700.0))
        .build_eframe(move |_| App::new(ai::Config { models: vec![first, other], footer: None, skills: Vec::new(), ghosts: None }, vec![tab], None));
    harness.run_steps(4);
    harness.get_by_role(egui::accesskit::Role::ComboBox).click();
    harness.run_steps(4);
    harness.get_by_label("Other").click();
    harness.run_steps(4);
    assert_eq!(doc(&harness).model, 1);
    shot(&mut harness, "7-model");
}

#[test]
fn ghosts_use_the_ghost_model_and_prose_the_picked_one() {
    let tab = Doc::open(temp("ghost-model")).unwrap();
    let mut small = ai::Model::new(ai::Backend::Cli { command: "sh".into(), args: vec!["-c".into(), r#"cat >/dev/null; printf '["Tiny"]'"#.into()] });
    small.name = "Small".into();
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1000.0, 700.0))
        .build_eframe(move |_| App::new(ai::Config { models: vec![fake_ai(), small], footer: None, skills: Vec::new(), ghosts: Some(1) }, vec![tab], None));
    harness.run();
    harness.get_by_role(egui::accesskit::Role::TextInput).type_text("Rust");
    harness.key_press(Key::Enter);
    wait_for(&mut harness, "suggestion: Tiny");
    assert_eq!(doc(&harness).model, 0, "the tab keeps its picked model");
    harness.key_press(Key::L);
    wait_for(&mut harness, "How it works");
}

#[test]
fn the_title_bar_keys_by_task_and_the_empty_map_hint() {
    let (mut harness, _) = harness("chrome");
    harness.run();
    assert!(shows(&harness, &format!("Mind-Me v{}", env!("CARGO_PKG_VERSION").rsplitn(2, '.').last().unwrap())), "the title bar names the app and version");
    harness.get_by_role(egui::accesskit::Role::TextInput).type_text("Rust");
    harness.key_press(Key::Enter);
    wait_for(&mut harness, "suggestion: Alpha");
    let hint = "Click a suggestion to keep it · Tab adds your own · M for more";
    assert!(shows(&harness, hint), "a fresh map says how to grow it");
    harness.get_by_label("Keys").click();
    harness.run();
    assert!(shows(&harness, "Asking the AI") && shows(&harness, "Dialogs and pictures") && shows(&harness, "In Discuss: send / new line"), "keys are grouped and include the ones that work in one place");
    shot(&mut harness, "25-keys");
    harness.key_press(Key::Escape);
    harness.get_by_label("suggestion: Alpha").click_accesskit();
    wait_for(&mut harness, "node: Alpha");
    assert!(!shows(&harness, hint), "the hint goes once the map has a branch");
}

#[test]
fn f1_opens_the_help_map_fresh_each_time() {
    let map = Map::parse(&super::help::text()).expect("the help map is a valid map");
    let md = map.to_markdown();
    for (_, keys) in KEYS {
        for (key, does) in *keys {
            assert!(md.contains(&format!("- {key}: {does}")), "the shortcuts branch lists {key}");
        }
    }
    let (mut harness, _) = harness("help");
    start(&mut harness, "Rust");
    harness.key_press(Key::F1);
    steps(&mut harness, 5);
    assert_eq!(doc(&harness).name(), "help.md");
    assert!(shows(&harness, "node: Mind-Me help") && doc(&harness).in_flight == 0, "it opens without asking the AI");
    assert_eq!(harness.state().docs.len(), 2, "beside the map, not over it");
    harness.key_press(Key::ArrowRight);
    harness.key_press(Key::Tab);
    type_branch(&mut harness, "Scribble");
    assert!(shows(&harness, "node: Scribble"));
    harness.key_press(Key::F1);
    steps(&mut harness, 5);
    assert!(!shows(&harness, "node: Scribble") && harness.state().docs.len() == 2, "a second F1 resets the same tab");
    assert!(!harness.state().recent.iter().any(|p| p.ends_with("help.md")), "help never joins the recent maps");
    shot(&mut harness, "27-help");
}

#[test]
fn feedback_opens_an_email_draft_with_the_text() {
    let (mut harness, _) = harness("feedback");
    start(&mut harness, "Rust");
    harness.get_by_label("Feedback").click();
    harness.run();
    assert!(harness.get_by_label("Write email").accesskit_node().is_disabled(), "nothing to send yet");
    let text = harness.query_all_by_role(egui::accesskit::Role::MultilineTextInput).find(|n| n.accesskit_node().placeholder() == Some("What happened, or what would you like?")).unwrap();
    text.click();
    harness.run();
    harness.event(egui::Event::Text("The map froze & I lost a node".into()));
    harness.run();
    shot(&mut harness, "feedback");
    harness.get_by_label("Write email").click_accesskit();
    harness.step();
    let urls: Vec<String> = harness.output().platform_output.commands.iter().filter_map(|c| match c {
        egui::OutputCommand::OpenUrl(u) => Some(u.url.clone()),
        _ => None,
    }).collect();
    assert_eq!(urls.len(), 1, "one draft: {urls:?}");
    assert!(urls[0].starts_with("mailto:feedback@mindme.cc?subject=Mind-Me%20feedback&body=The%20map%20froze%20%26%20I%20lost"), "{}", urls[0]);
    harness.run();
    assert!(!shows(&harness, "Write email") && harness.state().feedback.is_none(), "it closes once the draft opens");
}
