use super::*;

/// Screenshots for the landing page, in three themes, written only when `MINDME_SITE` names a folder:
/// `MINDME_SITE=site/img cargo test site_screenshots`.
#[test]
fn site_screenshots() {
    let Ok(dir) = std::env::var("MINDME_SITE") else { return };
    let map = "# Learn Rust\n> A systems language that is fast and memory-safe without a garbage collector. The compiler checks how every value is owned and borrowed.\n- Ownership\n  > Every value has one owner. When the owner goes out of scope, the value is dropped, so memory is freed with no collector.\n  - Borrowing\n    > A reference lends a value without taking it: many shared ones, or exactly one mutable one.\n  - Lifetimes\n    > How long a reference stays valid. The compiler infers most of them.\n- Cargo\n  > The build tool and package manager.\n  - Crates\n  - Workspaces\n- Error handling\n  > `Result` and `Option` make failure part of a function's type, and `?` passes it up.\n- Traits\n  > Shared behavior that types opt into, like interfaces.\n  - Generics\n";
    // A model that suggests what a real one would, under the name a real one shows.
    let reply = r#"printf '{"note": "Ownership.", "nodes": ["Move semantics", "Drop", "Smart pointers", "Copy types"]}'"#;
    let mut model = ai::Model::new(ai::Backend::Cli { command: "sh".into(), args: vec!["-c".into(), reply.into()] });
    model.name = "Claude Pro/Max".into();
    for (theme, light, name) in [("mindme", false, "mindme-dark"), ("tokyo-night", false, "tokyo-night"), ("catppuccin", true, "catppuccin-latte")] {
        let folder = std::env::temp_dir().join(format!("mindme-site-{}-{name}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("learn-rust.md");
        fs::write(&path, map).unwrap();
        let (doc, model) = (Doc::open(path).unwrap(), model.clone());
        let mut harness = Harness::builder().with_size(Vec2::new(1280.0, 800.0)).with_max_steps(8).build_eframe(move |_| {
            App::new(ai::Config { models: vec![model], footer: None, skills: Vec::new(), ghosts: None }, vec![doc], None)
        });
        (harness.state_mut().settings.theme, harness.state_mut().settings.light, harness.state_mut().settings.wiki) = (theme.into(), light, false);
        harness.run();
        harness.get_by_label("Show all").click();
        steps(&mut harness, 5);
        harness.get_by_label("node: Ownership").click_accesskit();
        steps(&mut harness, 250);
        harness.key_press(Key::M);
        wait_for(&mut harness, "suggestion: Smart pointers");
        steps(&mut harness, 250);
        harness.key_press(Key::F);
        harness.event(egui::Event::PointerGone);
        steps(&mut harness, 30);
        fs::create_dir_all(&dir).unwrap();
        let shot = harness.render().unwrap();
        shot.save(format!("{dir}/{name}.png")).unwrap();
        // The map around Ownership and its suggestions, small enough to show three side by side with readable labels.
        image::imageops::crop_imm(&shot, 400, 300, 520, 400).to_image().save(format!("{dir}/{name}-detail.png")).unwrap();
    }
}
