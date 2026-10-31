use super::*;

/// "Rust" with Alpha kept and current, fitted to the window. It runs at 60 frames a second so a multi-click
/// spans as many frames as on a real screen. Returns the root's screen point.
fn fitted(name: &str) -> (Harness<'static, App>, Pos2) {
    let opened = Doc::open(temp(name)).unwrap();
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1000.0, 700.0))
        .with_step_dt(1.0 / 60.0)
        .build_eframe(move |_| App::new(ai::Config { models: vec![fake_ai()], footer: None, skills: Vec::new(), ghosts: None }, vec![opened], None));
    start(&mut harness, "Rust");
    harness.get_by_label("suggestion: Alpha").click_accesskit();
    wait_for(&mut harness, "suggestion: Beta");
    steps(&mut harness, 200);
    harness.key_press(Key::F);
    steps(&mut harness, 3);
    let root = doc(&harness).map().unwrap().layout().nodes[0].id;
    let view = &harness.state().docs[0].view;
    let at = view.to_screen(view.graph.pos(&Body::Node(root)).unwrap());
    harness.hover_at(at);
    harness.step();
    (harness, at)
}

fn root(harness: &Harness<App>) -> NodeId {
    doc(harness).map().unwrap().layout().nodes[0].id
}

#[test]
fn double_click_a_node_to_rename_it() {
    let (mut harness, at) = fitted("double-click");
    click_at(&mut harness, at);
    steps(&mut harness, 3);
    click_at(&mut harness, at);
    steps(&mut harness, 6);
    harness.event(egui::Event::Text(" language".into()));
    harness.key_press(Key::Enter);
    steps(&mut harness, 5);
    assert!(shows(&harness, "node: Rust language"), "typing after a double-click edited the root's name");
    assert_eq!(current(&harness), root(&harness), "the double-clicked node became current");
}

#[test]
fn triple_click_a_node_for_more_ideas() {
    let (mut harness, at) = fitted("triple-click");
    let asked = doc(&harness).usage.input;
    for _ in 0..3 {
        click_at(&mut harness, at);
        steps(&mut harness, 3);
    }
    assert_eq!(current(&harness), root(&harness), "the triple-clicked node became current");
    assert_eq!(harness.ctx.memory(|m| m.focused()), None, "the name box the double-click focused gave up focus");
    assert!(shows(&harness, "Replace the suggestions for \"Rust\" with new ones?"), "More ideas asks first");
    harness.key_press(Key::Enter);
    for _ in 0..300 {
        if doc(&harness).usage.input > asked {
            break;
        }
        harness.step();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(doc(&harness).usage.input > asked, "asked the AI for fresh ghosts for the root");
    harness.key_press(Key::M);
    assert!(!shows(&harness, "node: RustM"), "M is a hotkey again, not typed into the name");
}

#[test]
fn the_path_to_the_current_node_is_lit() {
    let (mut harness, _) = fitted("lit");
    harness.key_press(Key::Escape);
    steps(&mut harness, 30);
    harness.get_by_label("suggestion: Beta").click_accesskit();
    steps(&mut harness, 120);
    shot(&mut harness, "23-lit-path");
    let image = harness.render().unwrap();
    // Inside the box's left padding, clear of its text and rounded corners.
    let fill = |label: &str| {
        let b = harness.get_by_label(label).accesskit_node().bounding_box().unwrap();
        let at = harness.state().docs[0].view.to_screen(Pos2::new(b.x0 as f32 + 3.0, ((b.y0 + b.y1) / 2.0) as f32));
        let [r, g, b, _] = image.get_pixel(at.x as u32, at.y as u32).0;
        Color32::from_rgb(r, g, b)
    };
    let near = |a: Color32, b: Color32| a.to_array().iter().zip(b.to_array()).all(|(x, y)| x.abs_diff(y) <= 2);
    assert!(near(fill("node: Rust"), mindme(false).node), "the root is on the path to Beta");
    assert!(near(fill("node: Beta"), mindme(false).node), "the current node is lit");
    assert!(near(fill("node: Alpha"), mindme(false).branch), "Alpha is off the path, so it is dimmer: {:?}", fill("node: Alpha"));
    harness.get_by_label("☀").click();
    steps(&mut harness, 3);
    shot(&mut harness, "23-lit-path-light");
    let root = harness.get_by_label("node: Rust").rect().height();
    let child = harness.get_by_label("node: Alpha").rect().height();
    assert!(root > child + 5.0, "the root's label is bigger than a branch's: {root} against {child}");
}

fn right_click(harness: &mut Harness<App>, pos: Pos2) {
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Secondary, pressed, modifiers: Modifiers::NONE });
        harness.step();
    }
    steps(harness, 2);
}

#[test]
fn right_click_a_node_to_copy_and_paste_branches() {
    let (mut harness, at) = fitted("menu");
    right_click(&mut harness, at);
    shot(&mut harness, "24-node-menu");
    assert!(shows(&harness, "Add child") && !shows(&harness, "Add sibling") && !shows(&harness, "Delete  Del"), "the root has no sibling and keeps its branches");
    harness.get_by_label("Copy  Ctrl+C").click();
    harness.step();
    assert_eq!(copied(&harness).as_deref(), Some("- Rust\n  > About it.\n  - Alpha\n    > About it.\n"));
    steps(&mut harness, 2);
    assert!(!shows(&harness, "Add child"), "the menu closed");

    let alpha = harness.get_by_label("node: Alpha").accesskit_node().bounding_box().unwrap();
    let alpha = harness.state().docs[0].view.to_screen(Pos2::new(((alpha.x0 + alpha.x1) / 2.0) as f32, ((alpha.y0 + alpha.y1) / 2.0) as f32));
    right_click(&mut harness, alpha);
    assert!(shows(&harness, "Add sibling") && shows(&harness, "Delete  Del"));
    harness.get_by_label("Paste  Ctrl+V").click();
    harness.step();
    // What eframe sends after it reads the clipboard the menu asked for.
    harness.event(egui::Event::Paste("Socks\n- Shoes\n  > Comfy ones.".into()));
    steps(&mut harness, 3);
    assert!(shows(&harness, "node: Socks") && shows(&harness, "node: Shoes"), "the pasted lines are branches of Alpha");
    assert!(doc(&harness).map().unwrap().to_markdown().ends_with("- Alpha\n  > About it.\n  - Socks\n  - Shoes\n    > Comfy ones.\n"));
    shot(&mut harness, "25-pasted");
    harness.key_press_modifiers(Modifiers::CTRL, Key::Z);
    steps(&mut harness, 3);
    assert!(!shows(&harness, "node: Socks") && !shows(&harness, "node: Shoes"), "one undo takes the paste back");
}

#[test]
fn right_click_add_child_opens_the_new_branch_box() {
    let (mut harness, at) = fitted("menu-add");
    right_click(&mut harness, at);
    harness.get_by_label("Add child").click();
    steps(&mut harness, 3);
    assert!(adding_box(&harness), "the New branch box opened");
    harness.event(egui::Event::Text("Gamma".into()));
    harness.key_press(Key::Enter);
    steps(&mut harness, 3);
    let map = doc(&harness).map().unwrap().to_markdown();
    assert!(map.contains("\n- Gamma\n"), "Gamma is a branch of the right-clicked root: {map}");
}

#[test]
fn right_click_an_outline_entry_to_rename_it() {
    let (mut harness, _) = fitted("menu-outline");
    let rust = harness.query_all_by_label("Rust").next().unwrap().rect().center();
    right_click(&mut harness, rust);
    harness.get_by_label("Rename").click();
    steps(&mut harness, 3);
    harness.event(egui::Event::Text(" language".into()));
    harness.key_press(Key::Enter);
    steps(&mut harness, 3);
    assert!(shows(&harness, "node: Rust language"), "typing after Rename edited the root's name: {}", doc(&harness).map().unwrap().to_markdown());
}

/// A press at `from`, a drag to `to` over several frames, and a release, the way a mouse sends them.
fn drag(harness: &mut Harness<App>, from: Pos2, to: Pos2) {
    harness.event(egui::Event::PointerMoved(from));
    harness.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    harness.step();
    for i in 1..=10 {
        harness.event(egui::Event::PointerMoved(from + (to - from) * i as f32 / 10.0));
        harness.step();
    }
    harness.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    steps(harness, 3);
}

fn screen_of(harness: &Harness<App>, label: &str) -> Pos2 {
    let b = harness.get_by_label(label).accesskit_node().bounding_box().unwrap();
    harness.state().docs[0].view.to_screen(Pos2::new(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32))
}

#[test]
fn drag_a_node_onto_another_to_move_it_and_cut_to_move_by_keyboard() {
    let path = temp("move");
    fs::write(&path, "# Rust\n- Alpha\n- Beta\n- Gamma\n").unwrap();
    let doc = Doc::open(path.clone()).unwrap();
    let mut harness = Harness::builder().with_size(Vec2::new(1000.0, 700.0)).with_step_dt(1.0 / 60.0).with_max_steps(8)
        .build_eframe(move |_| App::new(ai::Config { models: vec![fake_ai()], footer: None, skills: Vec::new(), ghosts: None }, vec![doc], None));
    harness.run();
    harness.key_press(Key::ArrowRight);
    steps(&mut harness, 200);
    harness.key_press(Key::F);
    steps(&mut harness, 5);
    let (beta, alpha) = (screen_of(&harness, "node: Beta"), screen_of(&harness, "node: Alpha"));
    drag(&mut harness, beta, alpha);
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Rust\n- Alpha\n  - Beta\n- Gamma\n", "Beta dropped onto Alpha moved under it");
    assert!(harness.state().docs[0].view.moving.is_none());

    harness.get_by_label("node: Gamma").click_accesskit();
    harness.run();
    harness.event(egui::Event::Cut);
    harness.step();
    assert_eq!(copied(&harness).as_deref(), Some("- Gamma\n"));
    steps(&mut harness, 3);
    assert!(!shows(&harness, "Delete \"Gamma\"?"), "a cut needs no yes");
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Rust\n- Alpha\n  - Beta\n");
    harness.get_by_label("node: Beta").click_accesskit();
    harness.run();
    harness.event(egui::Event::Paste("- Gamma\n".into()));
    steps(&mut harness, 3);
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Rust\n- Alpha\n  - Beta\n    - Gamma\n", "cut then paste moved it");
}

#[test]
fn drag_in_the_outline_to_reorder_or_move_under() {
    let path = temp("outline-move");
    fs::write(&path, "# Rust\n- Alpha\n- Beta\n- Gamma\n").unwrap();
    let doc = Doc::open(path.clone()).unwrap();
    let mut harness = Harness::builder().with_size(Vec2::new(1000.0, 700.0)).with_max_steps(8)
        .build_eframe(move |_| App::new(ai::Config { models: vec![fake_ai()], footer: None, skills: Vec::new(), ghosts: None }, vec![doc], None));
    harness.run();
    harness.key_press(Key::ArrowRight);
    steps(&mut harness, 30);
    // The outline is the left panel; the topic panel lists the same names on the right.
    let entry = |h: &Harness<App>, name: &str| h.query_all_by_label(name).map(|n| n.rect()).find(|r| r.right() < 230.0).unwrap();
    let (gamma, alpha) = (entry(&harness, "Gamma"), entry(&harness, "Alpha"));
    drag(&mut harness, gamma.center(), Pos2::new(alpha.center().x, alpha.top() + 2.0));
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Rust\n- Gamma\n- Alpha\n- Beta\n", "dropped on Alpha's top edge, Gamma goes before it");
    steps(&mut harness, 10);
    let (beta, alpha) = (entry(&harness, "Beta"), entry(&harness, "Alpha"));
    drag(&mut harness, beta.center(), alpha.center());
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Rust\n- Gamma\n- Alpha\n  - Beta\n", "dropped on Alpha's middle, Beta goes under it");
}
