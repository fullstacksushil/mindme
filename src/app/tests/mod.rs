use super::*;
use super::view::{Body, KEYS};
use super::theme::{self, Palette};
use super::widgets::{waiting, NO_WEB};
use crate::map::{Map, NodeId, Text};
use std::time::Duration;
use eframe::egui::{Color32, Pos2, Vec2};
use egui_kittest::{Harness, kittest::{NodeT, Queryable}};

mod start;
mod canvas;
mod inspector;
mod footer;
mod node;
mod modal;
mod site;
mod demo;

/// Answers at once, except about "Slow", which hangs until stopped: a small outline for a topic in plain text. Otherwise the same note and two nodes,
/// wrapped the way `claude -p --output-format json` wraps them, with tokens and a cost.
fn fake_ai() -> ai::Model {
    let reply = r#"case "$(cat)" in
        *"You are interviewing"*"So far:"*) printf 'Got it. {"done": true}' ;;
        *"You are interviewing"*) printf '{"question": "What is it for?", "recommended": "Systems work"}' ;;
        *"path: Slow"*) sleep 30 ;;
        *"Write like a pirate."*"in depth"*) printf 'Arr, **guided**.' ;;
        *"Show wide code"*) printf 'One function checks a route, which is the easy part. A second function tries every route.\n\n```js\nfunction distance(a, b) {\n  return Math.hypot(a.x - b.x, a.y - b.y);\n}\n// The easy part: check one route by adding up its legs, including the trip home.\n```\n' ;;
        *"Reply to the last message"*) printf 'Try **Box**.' ;;
        *"concrete examples"*) printf '## Examples\n- [Box](https://ok.example/box) owns its value.\n- [Rc](https://dead.example/rc) shares it.\n' ;;
        *"in depth"*) printf '```markdown\n## How it works\nDeep **detail**.\n```' ;;
        *"Map out"*) printf '# Rust\n> A **language**.\n- Ownership\n  > Who frees memory.\n  - Borrowing\n- Cargo\n' ;;
        *) printf '{"result": "{\\"note\\": \\"About it.\\", \\"nodes\\": [\\"Alpha\\", \\"Beta\\"]}", "usage": {"input_tokens": 10, "cache_read_input_tokens": 1990, "output_tokens": 20}, "total_cost_usd": 0.0125}' ;;
    esac"#;
    ai::Model::new(ai::Backend::Cli { command: "sh".into(), args: vec!["-c".into(), reply.into()] })
}

/// Mind-Me's own dark or light palette, the default theme's.
fn mindme(light: bool) -> Palette {
    theme::themes(None).into_iter().find(|t| t.name == theme::DEFAULT).unwrap().palette(light)
}

fn temp(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("mindme-{name}-{}.md", std::process::id()));
    let _ = fs::remove_file(&path);
    path
}

fn harness(name: &str) -> (Harness<'static, App>, PathBuf) {
    let path = temp(name);
    let doc = Doc::open(path.clone()).unwrap();
    // A theme or type scale change restyles every widget, which takes egui a frame or two more to settle.
    let harness = Harness::builder().with_size(Vec2::new(1000.0, 700.0)).with_max_steps(8).build_eframe(move |_| App::new(ai::Config { models: vec![fake_ai()], footer: None, skills: Vec::new(), ghosts: None }, vec![doc], None));
    (harness, path)
}

fn doc<'a>(harness: &'a Harness<App>) -> &'a Doc {
    &harness.state().docs[harness.state().active]
}

fn current(harness: &Harness<App>) -> NodeId {
    doc(harness).map().unwrap().layout().current
}

fn start(harness: &mut Harness<App>, idea: &str) {
    harness.run();
    harness.get_by_role(egui::accesskit::Role::TextInput).type_text(idea);
    harness.key_press(Key::Enter);
    wait_for(harness, "suggestion: Alpha");
}

/// Steps the app in real time until `label` is on screen. Replies arrive from a worker thread.
fn wait_for(harness: &mut Harness<App>, label: &str) {
    for _ in 0..300 {
        harness.step();
        if harness.query_all_by_label(label).next().is_some() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let labels: Vec<String> = harness.query_all(egui_kittest::kittest::by()).filter_map(|n| n.accesskit_node().label()).collect();
    panic!("{label:?} never appeared. On screen: {labels:?}");
}

/// Writes a screenshot to `MINDME_SHOTS`, when set, for a person to look at. Not an assertion.
fn shot(harness: &mut Harness<App>, name: &str) {
    let Ok(dir) = std::env::var("MINDME_SHOTS") else { return };
    fs::create_dir_all(&dir).unwrap();
    harness.render().unwrap().save(format!("{dir}/{name}.png")).unwrap();
}

fn steps(harness: &mut Harness<App>, n: usize) {
    for _ in 0..n {
        harness.step();
    }
}

fn shows(harness: &Harness<App>, label: &str) -> bool {
    harness.query_all_by_label(label).next().is_some()
}

/// Types into whichever box has focus, the way a person would after the app focused it.
fn type_branch(harness: &mut Harness<App>, text: &str) {
    harness.run();
    harness.event(egui::Event::Text(text.into()));
    harness.key_press(Key::Enter);
    harness.run();
}

fn adding_box(harness: &Harness<App>) -> bool {
    harness.query_all_by_role(egui::accesskit::Role::TextInput).any(|n| n.accesskit_node().placeholder() == Some("New branch"))
}

/// A press and release at a screen point, the way a mouse sends them.
fn click_at(harness: &mut Harness<App>, pos: Pos2) {
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        harness.step();
    }
}

fn copied(harness: &Harness<App>) -> Option<String> {
    harness.output().platform_output.commands.iter().find_map(|c| match c {
        egui::OutputCommand::CopyText(text) => Some(text.clone()),
        _ => None,
    })
}
