use super::fetch::{bitmaps, checkable, markdown, parse};
use super::*;

#[test]
fn links_encode_spaces_ampersands_and_unicode() {
    assert_eq!(query(&["Learn Rust", "Ownership"]), "Learn Rust Ownership");
    assert_eq!(query(&["A", "B", "C"]), "B C");
    assert_eq!(query(&["Root"]), "Root");
    assert_eq!(wikipedia_url("Tom & Jerry"), "https://en.wikipedia.org/wiki/Special:Search?search=Tom%20%26%20Jerry");
    assert_eq!(youtube_url("Café +1"), "https://www.youtube.com/results?search_query=Caf%C3%A9%20%2B1");
}

#[test]
fn links_come_from_markdown_angle_brackets_and_bare_text() {
    let note = "See [Rust](https://en.wikipedia.org/wiki/Rust_(programming_language)), <http://a.org/x> and https://b.org/y.\n\
Sources: [Book](https://doc.rust-lang.org/book/), [again](https://b.org/y), httpd, http:/broken";
    assert_eq!(links(note), ["https://en.wikipedia.org/wiki/Rust_(programming_language)", "http://a.org/x", "https://b.org/y", "https://doc.rust-lang.org/book/"]);
    assert!(links("No links here.").is_empty());
}

#[test]
fn a_search_reply_becomes_a_card() {
    // Trimmed from a real reply for "Rust Ownership" on 2026-10-29.
    let reply = r#"{"batchcomplete":true,"continue":{"gsroffset":1},"query":{"pages":[{"pageid":29414838,"ns":0,"title":"Rust (programming language)","index":1,
        "thumbnail":{"source":"https://thumb.wikimedia.org/rust.png","width":320,"height":320},
        "extract":"Rust is a general-purpose programming language.","fullurl":"https://en.wikipedia.org/wiki/Rust_(programming_language)"}]}}"#;
    let card = parse(&serde_json::from_str(reply).unwrap()).remove(0);
    assert_eq!(card.title, "Rust (programming language)");
    assert_eq!(card.extract, "Rust is a general-purpose programming language.");
    assert_eq!(card.url, "https://en.wikipedia.org/wiki/Rust_(programming_language)");
    assert_eq!(card.thumb.as_deref(), Some("https://thumb.wikimedia.org/rust.png"));
    assert!(parse(&serde_json::json!({ "batchcomplete": true })).is_empty());
}

#[test]
fn only_jpegs_and_pngs_are_pictures() {
    // Trimmed from a real `generator=images` reply for "Homeostasis" on 2026-10-29.
    let reply = r#"{"query":{"pages":[
        {"title":"File:Wiktionary-logo-en-v2.svg","imageinfo":[{"thumburl":"https://up.example/logo.png","mime":"image/svg+xml"}]},
        {"title":"File:Negative Feedback Gif.gif","imageinfo":[{"thumburl":"https://up.example/loop.gif","mime":"image/gif"}]},
        {"title":"File:XBio illustration.png","imageinfo":[{"thumburl":"https://up.example/xbio.png","mime":"image/png"}]}]}}"#;
    assert_eq!(bitmaps(&serde_json::from_str(reply).unwrap()), ["https://up.example/xbio.png"]);
    assert!(bitmaps(&serde_json::json!({ "batchcomplete": true })).is_empty());
}

#[test]
fn article_headings_become_markdown() {
    let text = "Rust is a language.\n\n\n== History ==\nBegun in 2006.\n\n=== Mozilla ===\nSponsored.\n\n== See also ==\n\n== Notes ==\n";
    assert_eq!(markdown(text), "Rust is a language.\n\n\n## History\n\nBegun in 2006.\n\n\n### Mozilla\n\nSponsored.\n\n\n## See also\n\n\n## Notes\n\n");
    assert_eq!(markdown("A = B is true.\n"), "A = B is true.\n\n");
}

#[test]
fn the_hit_that_fits_the_note_wins_over_the_top_hit() {
    // Trimmed from a real reply for "rust" on 2026-10-29: pages arrive out of search order.
    let reply = r#"{"query":{"pages":[
        {"index":1,"title":"Rust","extract":"Rust is an iron oxide, a usually reddish-brown oxide formed by the reaction of iron and oxygen.","fullurl":"https://en.wikipedia.org/wiki/Rust"},
        {"index":3,"title":"Rust (video game)","extract":"Rust is a 2018 multiplayer survival video game.","fullurl":"https://en.wikipedia.org/wiki/Rust_(video_game)"},
        {"index":2,"title":"Rust (programming language)","extract":"Rust is a general-purpose programming language that emphasizes performance, type safety, and concurrency.","fullurl":"https://en.wikipedia.org/wiki/Rust_(programming_language)"}]}}"#;
    let cards = parse(&serde_json::from_str(reply).unwrap());
    assert_eq!(cards.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(), ["Rust", "Rust (programming language)", "Rust (video game)"]);
    let note = "Rust is a systems programming language focused on memory safety, speed, and concurrency.";
    assert_eq!(best(&cards, "Rust", note).unwrap().title, "Rust (programming language)");
    assert_eq!(best(&cards, "Rust", "").unwrap().title, "Rust", "no note keeps the top hit");
    assert!(best(&[], "Rust", note).is_none());
}

#[test]
fn a_hit_that_misses_a_word_of_the_node_is_no_fit() {
    // Trimmed from a real reply for "Mind-Me Landing page" on 2026-10-30.
    let reply = r#"{"query":{"pages":[
        {"index":1,"title":"List of Knots Landing episodes","extract":"Knots Landing is an American prime time television soap opera.","fullurl":"https://en.wikipedia.org/wiki/List_of_Knots_Landing_episodes"},
        {"index":2,"title":"Black House (novel)","extract":"Black House is a 2001 novel by Stephen King and Peter Straub.","fullurl":"https://en.wikipedia.org/wiki/Black_House_(novel)"}]}}"#;
    let cards = parse(&serde_json::from_str(reply).unwrap());
    assert!(best(&cards, "Landing page", "Mind-Me Landing page").is_none(), "no hit is about a landing page");
    let page = Card { title: "Landing page".into(), extract: "A landing page is a web page.".into(), url: String::new(), thumb: None };
    let cards = [cards, vec![page]].concat();
    assert_eq!(best(&cards, "Landing page", "Mind-Me Landing page").unwrap().title, "Landing page");
    assert_eq!(best(&cards, "Go", "Go").unwrap().title, "List of Knots Landing episodes", "a name with no 4+ letter word keeps the top hit");
}

#[test]
fn no_fitting_hit_anywhere_on_the_path_shows_no_picture() {
    let card = |title: &str| Card { title: title.into(), extract: String::new(), url: String::new(), thumb: Some(format!("{title}.png")) };
    let mut lookup = Lookup::default();
    lookup.wiki.got.insert("Settings & Dev Console".into(), Ok(vec![card("Google Drive")]));
    lookup.wiki.got.insert("Settings & Dev Console Live Log Viewer".into(), Ok(vec![card("List of built-in macOS apps")]));
    lookup.images.got.insert("Google Drive".into(), vec!["drive.png".into()]);
    lookup.images.got.insert("List of built-in macOS apps".into(), vec!["mac.png".into()]);
    let topic = lookup.topic(&["Settings & Dev Console", "Live Log Viewer"], None, 0, &egui::Context::default());
    assert!(topic.pictures.is_empty(), "unrelated pictures stay out: {:?}", topic.pictures);
    assert!(topic.card.is_err(), "so the Wikipedia link searches in the browser");
}

#[test]
fn a_node_without_pictures_shows_its_nearest_ancestors() {
    let card = |title: &str| Card { title: title.into(), extract: String::new(), url: String::new(), thumb: None };
    let mut lookup = Lookup::default();
    lookup.wiki.got.insert("Rust".into(), Ok(vec![card("Rust")]));
    lookup.wiki.got.insert("Rust Alpha".into(), Ok(vec![card("Alpha")]));
    lookup.images.got.insert("Rust".into(), vec!["one.png".into(), "two.png".into()]);
    lookup.images.got.insert("Alpha".into(), Vec::new());
    let topic = lookup.topic(&["Rust", "Alpha"], Some("A note."), 3, &egui::Context::default());
    assert_eq!(topic.pictures, ["one.png", "two.png"]);
    let card = topic.card.unwrap();
    assert_eq!((card.title.as_str(), card.thumb.as_deref()), ("Alpha", Some("two.png")), "Alpha's own card, with the picture at the slide");
}

#[test]
fn the_sources_line_counts_links_by_state() {
    let mut lookup = Lookup::default();
    lookup.links.got.insert("https://a.org".into(), Link::Ok);
    lookup.links.got.insert("https://b.org".into(), Link::Dead("HTTP 404".into()));
    lookup.links.got.insert("https://c.org".into(), Link::Checking);
    let ctx = egui::Context::default();
    let sources = lookup.sources("https://a.org https://b.org https://c.org", &ctx);
    assert_eq!(sources.line(), "Sources: 3 links, 1 reachable, 1 dead, 1 checking");
    assert_eq!(sources.dead, [("https://b.org".to_owned(), "HTTP 404".to_owned())]);
    assert_eq!(lookup.sources("See https://a.org.", &ctx).line(), "Sources: 1 link, 1 reachable");
    lookup.links.got.insert("http://d.org".into(), Link::Unchecked);
    assert_eq!(lookup.sources("https://a.org http://d.org", &ctx).line(), "Sources: 2 links, 1 reachable, 1 not checked");
}

#[test]
fn only_https_links_to_named_hosts_are_checked() {
    for url in ["https://doc.rust-lang.org/book/", "https://en.wikipedia.org/wiki/Rust_(language)", "https://user@example.com:8443/x"] {
        assert!(checkable(url), "{url}");
    }
    let local = [
        "http://example.com/",
        "http://192.168.1.1/reboot",
        "https://192.168.1.1/",
        "https://127.1/",
        "https://2130706433/",
        "https://[::1]:8080/",
        "https://localhost:3000/",
        "https://LOCALHOST./",
        "https://app.localhost/",
        "https://evil@10.0.0.1/",
        "https:///nohost",
    ];
    for url in local {
        assert!(!checkable(url), "{url}");
    }
}

#[test]
fn web_images_fetch_once_and_report_a_failure() {
    use eframe::egui::load::{BytesLoader, BytesPoll, LoadError};
    let (images, ctx) = (WebImages::default(), egui::Context::default());
    assert!(matches!(images.load(&ctx, "file:///tmp/x.png"), Err(LoadError::NotSupported)), "only web addresses");
    let uri = "https://example.com/x.png";
    assert!(matches!(images.load(&ctx, uri), Ok(BytesPoll::Pending { .. })));
    for _ in 0..100 {
        if let Err(LoadError::Loading(e)) = images.load(&ctx, uri) {
            assert_eq!(e, "offline in tests");
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("the fetch never finished");
}
