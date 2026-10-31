use super::{tidy, Map, NodeId, Text};

impl Map {
    /// Reads a map written by `to_markdown` or by hand. `None` when there is no `#` title line.
    /// A `> ` line, or any other non-bullet line, is one line of a note. The note belongs to the nearest
    /// earlier bullet with smaller indent, or to the root.
    pub fn parse(md: &str) -> Option<Map> {
        let mut lines = md.lines().skip_while(|l| !l.starts_with('#'));
        let mut map = Map::new(Text::new(lines.next()?.trim_start_matches('#'))?);
        let root = map.current;
        // Open ancestors, indents strictly increasing from bottom to top.
        let mut stack: Vec<(usize, NodeId)> = Vec::new();
        let mut brief: Option<String> = None;
        let mut in_brief = false;
        for line in lines {
            if line == BRIEF_START && stack.is_empty() && brief.is_none() {
                (in_brief, brief) = (true, Some(String::new()));
                continue;
            }
            if in_brief {
                in_brief = line != BRIEF_END;
                if let Some(brief) = brief.as_mut().filter(|_| in_brief) {
                    *brief += &format!("{line}\n");
                }
                continue;
            }
            let indent = line.chars().take_while(|c| c.is_whitespace()).count();
            let body = line.trim_start();
            if body.is_empty() {
                continue;
            }
            let quoted = body.strip_prefix('>').map(|q| q.strip_prefix(' ').unwrap_or(q));
            let bullet = ["- ", "* ", "+ "].iter().find_map(|m| body.strip_prefix(m));
            while stack.last().is_some_and(|&(i, _)| i >= indent) {
                stack.pop();
            }
            let owner = stack.last().map_or(root, |&(_, id)| id);
            match bullet {
                Some(text) => {
                    let Some(text) = Text::new(text) else { continue };
                    let id = map.add(owner, text);
                    stack.push((indent, id));
                }
                None => {
                    let line = quoted.unwrap_or(body).trim_end();
                    let note = &mut map.nodes.iter_mut().find(|n| n.id == owner).unwrap().note;
                    *note = Some(note.take().map_or(line.to_owned(), |n| format!("{n}\n{line}")));
                }
            }
        }
        for node in &mut map.nodes {
            node.note = node.note.take().and_then(tidy);
        }
        map.set_brief(brief);
        Some(map)
    }

    /// `# root`, then each branch as a bullet indented 2 spaces per level, depth first in sibling order.
    /// A text starting with `#` always follows `- `, so it never reads as a heading.
    pub fn to_markdown(&self) -> String {
        let root = &self.nodes[0];
        let mut out = format!("# {}\n", root.text.as_str());
        if let Some(brief) = &self.brief {
            out += &format!("{BRIEF_START}\n{brief}\n{BRIEF_END}\n");
        }
        quote(root.note.as_deref(), "", &mut out);
        bullets(self, root.id, 0, &mut out);
        out
    }

    /// `id` as a bullet, its note quoted under it, then its branches: what Copy puts on the clipboard.
    pub fn subtree(&self, id: NodeId) -> String {
        let Some(node) = self.node(id) else { return String::new() };
        let mut out = format!("- {}\n", node.text.as_str());
        quote(node.note.as_deref(), "  ", &mut out);
        bullets(self, id, 1, &mut out);
        out
    }

    /// Adds pasted text under the current node, merged the way Map out merges. A whole map, from its `#` title,
    /// becomes one branch. Otherwise bullets keep their nesting, `> ` lines are notes and any other line is a branch.
    pub(super) fn paste(&mut self, text: &str) {
        let into = self.current;
        if text.trim_start().starts_with('#')
            && let Some(tree) = Map::parse(text)
        {
            let root = &tree.nodes[0];
            let id = self.children(into).find(|n| n.text.same(&root.text)).map(|n| n.id);
            let id = id.unwrap_or_else(|| self.add(into, root.text.clone()));
            return self.graft(&tree, root.id, id);
        }
        let mut md = String::from("# pasted\n");
        for line in text.lines() {
            let body = line.trim_start();
            let kept = body.is_empty() || body.starts_with('>') || ["- ", "* ", "+ "].iter().any(|m| body.starts_with(m));
            md += &if kept { format!("{line}\n") } else { format!("{}- {body}\n", &line[..line.len() - body.len()]) };
        }
        let tree = Map::parse(&md).unwrap();
        self.graft(&tree, tree.nodes[0].id, into);
    }
}

fn bullets(map: &Map, id: NodeId, depth: usize, out: &mut String) {
    for child in map.children(id) {
        *out += &format!("{}- {}\n", "  ".repeat(depth), child.text.as_str());
        quote(child.note.as_deref(), &"  ".repeat(depth + 1), out);
        bullets(map, child.id, depth + 1, out);
    }
}

/// The brief is saved as an HTML comment under the title, which Markdown viewers hide.
const BRIEF_START: &str = "<!-- brief";
const BRIEF_END: &str = "-->";


/// Writes each note line as `> ` at `indent`. A blank line becomes a bare `>`, so paragraphs survive.
fn quote(note: Option<&str>, indent: &str, out: &mut String) {
    for line in note.into_iter().flat_map(str::lines) {
        *out += &if line.is_empty() { format!("{indent}>\n") } else { format!("{indent}> {line}\n") };
    }
}
