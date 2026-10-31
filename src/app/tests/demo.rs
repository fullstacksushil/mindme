use super::*;
use eframe::egui::{Event, PointerButton};

/// Frames for the narrated demo video, written only when `MINDME_DEMO` names a folder:
/// `MINDME_DEMO=~/Videos/mindme-demo/rec/mindme cargo test --release demo_frames`. The video scripts compose them.
#[test]
fn demo_frames() {
    let Ok(dir) = std::env::var("MINDME_DEMO") else { return };
    let mut rec = Rec { h: demo_harness("stoicism.md"), dir: PathBuf::from(dir), n: 0, at: Pos2::new(900.0, 600.0), ripple: 0, frames: Vec::new(), scenes: Vec::new() };
    fs::create_dir_all(&rec.dir).unwrap();

    rec.scene("idea", |r| {
        r.hold(0.8);
        r.glide(|h| widget(h, egui::accesskit::Role::TextInput), 0.8);
        r.click();
        r.type_text("Stoicism");
        r.hold(0.3);
        r.key(Key::Enter);
    });
    rec.scene("ghosts", |r| {
        r.until("suggestion: Virtue");
        r.glide(|_| Pos2::new(900.0, 640.0), 0.6);
        r.hold(3.0);
    });
    rec.scene("keep", |r| {
        r.glide(|h| screen_of(h, "suggestion: Virtue"), 0.9);
        r.click();
        r.until("suggestion: Courage");
        r.hold(2.5);
    });
    rec.scene("grow", |r| {
        r.glide(|h| screen_of(h, "suggestion: Courage"), 0.9);
        r.click();
        r.until("node: Courage");
        r.hold(1.0);
        // The root has scrolled off the canvas by now, so step back through the outline, which is drawn first.
        r.glide(|h| outline(h, "Stoicism"), 0.9);
        r.click();
        r.until("suggestion: Marcus Aurelius");
        r.hold(1.5);
        r.glide(|h| screen_of(h, "suggestion: Marcus Aurelius"), 0.9);
        r.click();
        r.until("node: Marcus Aurelius");
        r.glide(|_| Pos2::new(1100.0, 650.0), 0.6);
        r.key(Key::Escape);
        r.key(Key::F);
        r.hold(2.5);
    });

    rec.h = demo_harness("sourdough.md");
    rec.at = Pos2::new(900.0, 600.0);
    rec.scene("mapout", |r| {
        r.hold(0.5);
        r.glide(|h| widget(h, egui::accesskit::Role::TextInput), 0.7);
        r.click();
        r.type_text("Sourdough bread");
        r.glide(|h| centre(h, "Map out"), 0.7);
        r.click();
        r.until("node: Sourdough bread");
        r.hold(0.8);
        r.until("Show all");
        r.glide(|h| centre(h, "Show all"), 0.8);
        r.click();
        r.hold(1.5);
        r.glide(|_| Pos2::new(1100.0, 650.0), 0.5);
        r.key(Key::F);
        r.hold(2.5);
    });
    rec.scene("note", |r| {
        r.glide(|h| screen_of(h, "node: Starter"), 0.9);
        r.click();
        r.hold(4.0);
    });
    rec.h = demo_harness("sourdough.md");
    rec.scene("file", |r| {
        r.hold(0.5);
        r.glide(|h| screen_of(h, "node: Sourdough bread"), 0.8);
        r.click();
        r.until("node: Baking");
        r.hold(1.0);
        r.glide(|h| screen_of(h, "node: Baking"), 0.9);
        r.click();
        r.hold(3.5);
    });

    let frames: Vec<_> = rec.frames.iter().map(|(f, t)| serde_json::json!([f, t])).collect();
    fs::write(rec.dir.join("capture.json"), serde_json::json!({ "frames": frames, "scenes": rec.scenes }).to_string()).unwrap();
}

const FPS: f32 = 30.0;
const PPP: f32 = 1.5;

/// A model that answers the way a real one would, so the video shows real-looking branches.
fn demo_ai() -> ai::Model {
    let reply = r#"prompt="$(cat)"; case "$prompt" in
        *'Map out'*) cat <<'EOF' ;;
# Sourdough bread
> Bread raised by a wild culture of yeast and lactic acid bacteria instead of packet yeast. The bacteria give it its **sour taste** and its keeping quality.
- Starter
  > Flour and water left to ferment until wild yeast and bacteria take hold. Fed daily, a healthy starter **doubles in 4 to 8 hours**.
  - Feeding ratio
  - Hydration
- Dough
  > Flour, water, salt and a spoon of active starter.
  - Autolyse
  - Stretch and fold
  - Bulk fermentation
- Shaping
  > Shaping builds surface tension, so the loaf rises up rather than out.
  - Bench rest
- Baking
  > A hot covered pot traps steam, so the crust stays soft long enough for the loaf to spring.
  - Dutch oven
  - Scoring
EOF
        *'Virtue > Courage'*) printf '{"note": "Courage is acting rightly in spite of fear or pain.", "nodes": ["Facing fear", "Endurance", "Moral courage"]}' ;;
        *'path: Stoicism > Virtue'*) printf '{"note": "For the Stoics, virtue is the only true good.", "nodes": ["Wisdom", "Courage", "Justice", "Temperance"]}' ;;
        *'path: Stoicism >'*) printf '{"note": "A core Stoic idea.", "nodes": ["Key texts", "Daily practice", "Modern use"]}' ;;
        *) printf '{"note": "An ancient Greek philosophy of living well by focusing on what you can control.", "nodes": ["Dichotomy of control", "Virtue", "Marcus Aurelius", "Negative visualization", "Modern Stoicism"]}' ;;
    esac"#;
    let mut model = ai::Model::new(ai::Backend::Cli { command: "sh".into(), args: vec!["-c".into(), reply.into()] });
    model.name = "Claude Pro/Max".into();
    model
}

/// The map in `name`, in a fresh folder, so the tab shows a plain file name.
fn demo_harness(name: &str) -> Harness<'static, App> {
    let folder = std::env::temp_dir().join(format!("mindme-demo-{}", std::process::id()));
    fs::create_dir_all(&folder).unwrap();
    let doc = Doc::open(folder.join(name)).unwrap();
    let mut h = Harness::builder().with_size(Vec2::new(1280.0, 720.0)).with_pixels_per_point(PPP).with_step_dt(1.0 / FPS).with_max_steps(8)
        .build_eframe(move |_| App::new(ai::Config { models: vec![demo_ai()], footer: None, skills: Vec::new(), ghosts: None }, vec![doc], None));
    h.state_mut().settings.wiki = false;
    h.run();
    h
}

/// Accesskit reports physical pixels, so divide by the scale to get points.
fn centre(h: &Harness<App>, label: &str) -> Pos2 {
    let b = h.get_by_label(label).accesskit_node().bounding_box().unwrap();
    Pos2::new((b.x0 + b.x1) as f32, (b.y0 + b.y1) as f32) / (2.0 * PPP)
}

/// A canvas widget's centre on screen: inside the Scene, accesskit reports canvas coordinates.
fn screen_of(h: &Harness<App>, label: &str) -> Pos2 {
    h.state().docs[0].view.to_screen(centre(h, label))
}

fn outline(h: &Harness<App>, label: &str) -> Pos2 {
    let b = h.query_all_by_label(label).next().unwrap().accesskit_node().bounding_box().unwrap();
    Pos2::new((b.x0 + b.x1) as f32, (b.y0 + b.y1) as f32) / (2.0 * PPP)
}

fn widget(h: &Harness<App>, role: egui::accesskit::Role) -> Pos2 {
    let b = h.get_by_role(role).accesskit_node().bounding_box().unwrap();
    Pos2::new(b.x0 as f32 + 60.0, (b.y0 + b.y1) as f32 / 2.0) / PPP
}

/// Steps the app one frame at a time and saves each frame with a drawn cursor, since a headless render has none.
struct Rec {
    h: Harness<'static, App>,
    dir: PathBuf,
    n: usize,
    at: Pos2,
    ripple: usize,
    frames: Vec<(String, f32)>,
    scenes: Vec<serde_json::Value>,
}

impl Rec {
    fn time(&self) -> f32 {
        self.n as f32 / FPS
    }

    fn scene(&mut self, id: &str, body: impl FnOnce(&mut Self)) {
        let start = self.time();
        body(self);
        self.scenes.push(serde_json::json!({ "id": id, "start": start, "end": self.time() }));
    }

    fn frame(&mut self) {
        self.h.step();
        let mut img = self.h.render().unwrap();
        if self.ripple > 0 {
            ring(&mut img, self.at * PPP, 14.0 + 3.0 * (10 - self.ripple) as f32);
            self.ripple -= 1;
        }
        cursor(&mut img, self.at * PPP);
        let name = format!("f{:05}.png", self.n);
        img.save(self.dir.join(&name)).unwrap();
        self.frames.push((name, self.time()));
        self.n += 1;
    }

    fn hold(&mut self, secs: f32) {
        for _ in 0..(secs * FPS) as usize {
            self.frame();
        }
    }

    /// Frames until `label` is on screen. Replies come from a worker thread in real time.
    fn until(&mut self, label: &str) {
        for _ in 0..300 {
            if shows(&self.h, label) {
                return;
            }
            self.frame();
            std::thread::sleep(Duration::from_millis(10));
        }
        let labels: Vec<String> = self.h.query_all(egui_kittest::kittest::by()).filter_map(|n| n.accesskit_node().label()).collect();
        panic!("{label:?} never appeared. On screen: {labels:?}");
    }

    /// Eases the pointer to a target that may move, such as a settling node.
    fn glide(&mut self, to: impl Fn(&Harness<App>) -> Pos2, secs: f32) {
        let (from, n) = (self.at, (secs * FPS) as usize);
        for i in 1..=n {
            let t = i as f32 / n as f32;
            self.at = from + (to(&self.h) - from) * (t * t * (3.0 - 2.0 * t));
            self.h.event(Event::PointerMoved(self.at));
            self.frame();
        }
    }

    fn click(&mut self) {
        self.hold(0.15);
        self.ripple = 10;
        for pressed in [true, false] {
            self.h.event(Event::PointerButton { pos: self.at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
            self.frame();
        }
        self.hold(0.2);
    }

    fn key(&mut self, key: Key) {
        self.h.key_press(key);
        self.frame();
    }

    fn type_text(&mut self, text: &str) {
        for c in text.chars() {
            self.h.event(Event::Text(c.into()));
            self.frame();
            self.frame();
        }
    }
}

/// An arrow pointer, black with a white edge, its tip at `tip`.
fn cursor(img: &mut image::RgbaImage, tip: Pos2) {
    let s = PPP * 1.4;
    let arrow = [(0.0, 0.0), (0.0, 17.0), (4.0, 13.0), (7.0, 20.0), (9.5, 19.0), (6.5, 12.0), (12.0, 12.0)].map(|(x, y)| (tip.x + x * s, tip.y + y * s));
    let inside = |x: f32, y: f32| {
        let mut odd = false;
        for i in 0..arrow.len() {
            let ((x1, y1), (x2, y2)) = (arrow[i], arrow[(i + 1) % arrow.len()]);
            if (y1 > y) != (y2 > y) && x < x1 + (y - y1) * (x2 - x1) / (y2 - y1) {
                odd = !odd;
            }
        }
        odd
    };
    for y in tip.y as u32..(tip.y + 22.0 * s) as u32 {
        for x in tip.x as u32..(tip.x + 14.0 * s) as u32 {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            if x >= img.width() || y >= img.height() || !inside(fx, fy) {
                continue;
            }
            let edge = [(-2.0, 0.0), (2.0, 0.0), (0.0, -2.0), (0.0, 2.0)].iter().any(|(dx, dy)| !inside(fx + dx, fy + dy));
            img.put_pixel(x, y, if edge { image::Rgba([255, 255, 255, 255]) } else { image::Rgba([20, 20, 20, 255]) });
        }
    }
}

/// The click ripple: a thin accent ring around the pointer.
fn ring(img: &mut image::RgbaImage, at: Pos2, r: f32) {
    for y in (at.y - r - 3.0).max(0.0) as u32..((at.y + r + 3.0) as u32).min(img.height()) {
        for x in (at.x - r - 3.0).max(0.0) as u32..((at.x + r + 3.0) as u32).min(img.width()) {
            if ((x as f32 - at.x).hypot(y as f32 - at.y) - r).abs() < 2.0 {
                img.put_pixel(x, y, image::Rgba([230, 180, 90, 255]));
            }
        }
    }
}
