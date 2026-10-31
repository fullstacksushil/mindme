# mindme

A small desktop mind map. Type one idea, and the AI suggests branches around it. Click a suggestion to keep it. Or type a topic and map it out whole at once. Maps are plain Markdown files.

## Run

Ready-to-run builds for macOS and Linux are free during the beta. From 1.0 they cost $20 once, for every 1.x release. Building it yourself is free, always:

```sh
# Linux needs these headers: libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev
cargo build --release
./target/release/mindme rust.md go.md
```

Each file opens in its own tab. With no path, it opens `map.md`.

## Pick a model

With no model set, the app opens on a setup screen: Claude Pro/Max (when `claude` is installed), an API key (Anthropic, OpenAI, OpenRouter or any OpenAI-style URL), Ollama (when it is running), or any command. Test checks that it answers, and Save writes it to `~/.config/mindme/config.json`. Settings, Models renames, removes and adds models later. Or set one by hand:

| Backend | Set |
| --- | --- |
| Any CLI that reads a prompt on stdin | `MINDME_CMD="claude -p"`, `MINDME_CMD="ollama run llama3.2"` or `MINDME_CMD="llm"` |
| Claude Code, lean, with web access for mapping out (recommended) | `MINDME_CMD="claude -p --output-format json --tools WebSearch,WebFetch --allowedTools WebSearch,WebFetch --strict-mcp-config --setting-sources local --disable-slash-commands"` |
| Local Ollama over HTTP | `MINDME_MODEL=llama3.2` |
| Any OpenAI-style API | `MINDME_URL=<chat completions URL> MINDME_MODEL=<model> MINDME_KEY=<key>` |

`MINDME_CMD` is split on spaces and never run through a shell, so quoted arguments do not work.

Plain `claude -p` sends Claude Code's every tool, MCP server, plugin and skill listing with each request, about 7 times the input tokens of the lean command above. With an `ANTHROPIC_API_KEY` you can use `claude -p --bare --output-format json` instead. `--bare` ignores a claude.ai login, so with a subscription it only says "Not logged in".

To pick between several models at the start of each tab, and to see an approximate cost in the footer, list them in `~/.config/mindme/config.json`:

```json
{
  "models": [
    { "name": "Claude", "cmd": "claude -p --output-format json" },
    { "name": "GPT", "url": "https://api.openai.com/v1/chat/completions", "model": "gpt-5",
      "key_env": "OPENAI_API_KEY", "price": { "in": 1.25, "out": 10, "cached": 0.125 } }
  ],
  "footer": ["model", "in", "out", "total", "cached", "cost"]
}
```

To send ghost suggestions to a cheaper model, add `"ghosts": "<a model's name>"`. Mapping out, Learn more and Discuss keep using the tab's model.

Writing guides (`"skills": [{ "name", "file" }]`) go ahead of every prose request in full. Point them at short digests of the rules, about 200 words, rather than whole skill files.

To choose between Claude models, give each its own entry, such as `{ "name": "Claude Opus", "cmd": "claude -p --model opus --output-format json" }` and the same with `--model haiku`. The footer shows the model that actually answered.

Prices are dollars per million tokens. `claude -p --output-format json` reports its own tokens and cost. The footer's "…" menu hides or shows each figure.

## Use

| Do | How |
| --- | --- |
| Brainstorm from an idea | Type it, Enter |
| Map out a whole topic | Type it, Ctrl+Enter |
| Open a node one level | Click it, or Right or Enter |
| Fold it again | Left or Enter, or the outline's arrow. The + / − button on top of the outline shows or collapses everything |
| Walk the tree | Up and Down |
| Keep a suggestion | Click it, or Down to it then Enter |
| Add a branch yourself | Tab or + Add beside Branches, type, Enter. Clear Auto-suggest on the start screen to build a map with no AI |
| More suggestions | M or More ideas. With Auto-suggest off (the default for a mapped-out topic), this is the only way the AI runs |
| Go to the parent | Esc |
| Delete a leaf | Delete |
| Rename | Double-click a node, or edit the name in the topic panel |
| More ideas on a node | Triple-click it |
| Node menu | Right-click a node on the canvas or in the outline |
| Copy or paste a branch | Ctrl+C copies the current node and everything under it as Markdown, Ctrl+V adds the clipboard under it |
| Undo or redo | Ctrl+Z, and Ctrl+Shift+Z or Ctrl+Y. Up to 100 steps per tab, while the tab is open |
| Write a note | Edit in the topic panel. Markdown works |
| Go deeper on a node | Learn more in the topic panel, or `L`. The AI's in-depth explanation is added under the note |
| Get interviewed before the map starts | Tick Grill me on the start screen. Your answers go with every AI request for the map |
| Map out a node | Map out in the topic panel, or `G`. The branches merge under the node |
| Get examples of a node | Examples in the topic panel, or `E`. Two or three examples are added under the note |
| Talk it over | Discuss at the bottom of the topic panel. Add to note keeps a reply |
| Look it up | Wikipedia or YouTube in the topic panel header. Wikipedia opens in its own tab |
| Check a note's sources | The line under the note counts its links and lists dead ones in red. A live link does not prove the page supports the note |
| Move things | Drag nodes, drag the background, Ctrl+scroll or + and - in the canvas corner to zoom, F or Fit to fit |
| Spread the graph out | Graph in the canvas corner: text size, link thickness, center, repel and link force, link distance, Rearrange |
| Hide a panel | « or » in its header. The button to bring it back sits in the canvas corner |
| Graph only | `\` hides both panels, `\` again brings them back |
| Read a note | R or Read in the topic panel header: the topic panel alone in a centered column. R or Esc leaves |
| Open a map | Ctrl+O, or Open… in the + menu or on the start screen. You can pick several |
| Make a new map | Ctrl+N, or New map… |
| Export a map | The + menu: Copy map as Markdown, Copy map as JSON, or Export… |
| Reopen a recent map | The + menu, or the start screen of an empty tab. The last 10 are kept |
| Tabs | Ctrl+T types a path, Ctrl+Tab switches |
| Settings | Ctrl+, or Settings in the footer: theme, size, fonts, notes, graph and footer |
| Change the theme | Settings, Appearance. On Omarchy it follows the Omarchy theme by default. Your own themes go in `~/.config/mindme/themes/<name>.toml`, in Omarchy's `colors.toml` format |
| Light or dark | The sun or moon button in the footer switches the theme's dark and light versions |
| Bigger or smaller | Ctrl+= and Ctrl+-, Ctrl+0 resets, or Size in Settings |
| Stop an AI call | Stop beside the timer of anything waiting |
| All keys | Keys in the footer |

## File format

```markdown
# Rust
> A systems language with **no garbage collector**.
- Ownership
  > Each value has one owner.
  - Borrowing
```

The title is the root, bullets are branches, and `> ` lines are the note of the bullet above them. The app saves after every change. Edit the file in any editor while the app is closed.

## License and name

Mind-Me is free software under the GNU General Public License, version 3 or later: see `LICENSE`. Third-party licenses are in `THIRD-PARTY-NOTICES.md`. The license covers the code, not the name: a fork you share must use another name and icon.

## Releasing

1. Regenerate the notices when dependencies change. `cargo about` (install with `cargo install cargo-about --features cli`) also fails if a dependency's license is not in `about.toml`'s accepted list:
   `{ cat packaging/notices-head.md; cargo about generate about.hbs; } > THIRD-PARTY-NOTICES.md`
2. Bump `version` in `Cargo.toml`, commit, then `git tag v0.1.0 && git push --tags`.
3. The `release` workflow builds the macOS dmg (Apple silicon, unsigned during the beta) and the Linux AppImage and tarball, and publishes them on a GitHub Release, which the website's download buttons point at.

## Docs

`GLOSSARY.md` has the words the app and the code use.
