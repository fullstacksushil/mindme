use super::{links, Card, Images, Link, Links, Wiki};
use eframe::egui;

/// Everything the topic panel fetches, per tab.
#[derive(Default)]
pub struct Lookup {
    pub wiki: Wiki,
    pub images: Images,
    pub links: Links,
}

/// The current node's Wikipedia hit and its pictures, with the ancestor fallback applied.
pub struct Topic {
    pub card: Result<Card, &'static str>,
    pub pictures: Vec<String>,
}

/// The note's links counted by state. `dead` holds each dead URL and why.
pub struct Sources {
    pub total: usize,
    pub reachable: usize,
    pub dead: Vec<(String, String)>,
    pub checking: usize,
    pub unchecked: usize,
}

impl Lookup {
    /// The hit for the path that best fits the note, and its pictures. `card.thumb` is the picture at `slide`,
    /// so the article tab opens on the picture showing.
    pub fn topic(&mut self, path: &[&str], note: Option<&str>, slide: usize, ctx: &egui::Context) -> Topic {
        let context = match note {
            Some(text) => format!("{} {text}", path.join(" ")),
            None => path.join(" "),
        };
        let (card, mut pictures) = self.pictures(path, &context, ctx);
        // A node with no pictures of its own, or none yet, borrows its nearest ancestor's.
        for end in (1..path.len()).rev() {
            if !pictures.is_empty() {
                break;
            }
            pictures = self.pictures(&path[..end], &path[..end].join(" "), ctx).1;
        }
        let card = card.map(|card| Card { thumb: pictures.get(slide % pictures.len().max(1)).cloned(), ..card });
        Topic { card, pictures }
    }

    /// The hit's thumbnail, then the article's own images and Commons.
    fn pictures(&mut self, path: &[&str], context: &str, ctx: &egui::Context) -> (Result<Card, &'static str>, Vec<String>) {
        let card = self.wiki.card(path, context, ctx).cloned();
        let mut pictures: Vec<String> = Vec::new();
        if let Ok(card) = &card {
            pictures.extend(card.thumb.clone());
            for picture in self.images.images(&card.title, ctx) {
                if !pictures.contains(picture) {
                    pictures.push(picture.clone());
                }
            }
        }
        (card, pictures)
    }

    pub fn sources(&mut self, note: &str, ctx: &egui::Context) -> Sources {
        let urls = links(note);
        let mut sources = Sources { total: urls.len(), reachable: 0, dead: Vec::new(), checking: 0, unchecked: 0 };
        for url in urls {
            match self.links.link(url, ctx) {
                Link::Ok => sources.reachable += 1,
                Link::Dead(why) => sources.dead.push((url.to_owned(), why.clone())),
                Link::Checking => sources.checking += 1,
                Link::Unchecked => sources.unchecked += 1,
            }
        }
        sources
    }
}

impl Sources {
    pub fn line(&self) -> String {
        let mut line = format!("Sources: {} link{}, {} reachable", self.total, if self.total == 1 { "" } else { "s" }, self.reachable);
        if !self.dead.is_empty() {
            line += &format!(", {} dead", self.dead.len());
        }
        if self.unchecked > 0 {
            line += &format!(", {} not checked", self.unchecked);
        }
        if self.checking > 0 {
            line += &format!(", {} checking", self.checking);
        }
        line
    }
}
