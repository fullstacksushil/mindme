# mindme

A mind map where you grow ideas from one central idea, keeping the AI suggestions you choose. Only what you keep is saved.

## The map

**Map**:
A tree of nodes grown from one central idea, saved as one Markdown file.
_Avoid_: Document, mind map file

**Root**:
The node a map grows from: its central idea and title.
_Avoid_: Central node, topic

**Node**:
One idea in a map, a single line of text with an optional note.
_Avoid_: Item, entry, bubble

**Branch**:
A node seen as the child of another node.
_Avoid_: Child (in prose), sub-node

**Subtree**:
A node together with everything under it.
_Avoid_: Branch (for the whole subtree)

**Note**:
The Markdown explanation attached to a node.
_Avoid_: Description, summary, explanation

**Current node**:
The one node the user is working on; ghosts, the topic panel and every AI action are about it.
_Avoid_: Selected node, focused node, active node

**Fold**:
Hiding a node's subtree below it from view. A view choice, never part of the map.
_Avoid_: Collapse (except the "Collapse all" button), hide

## Suggestions

**Ghost**:
A node the AI suggests under the current node that is not part of the map until kept.
_Avoid_: Suggestion, candidate, proposal

**Keep**:
Turning a ghost into a real node of the map.
_Avoid_: Accept, pick, adopt

**More ideas**:
Asking for a fresh set of ghosts in place of the ones showing.
_Avoid_: Reroll, refresh, regenerate

**Passed-over ghost**:
A ghost that was showing when the user asked for more ideas, and is never suggested again for that node.
_Avoid_: Rejected ghost, discarded ghost

**Auto-suggest**:
Whether a node that becomes current asks for ghosts by itself.
_Avoid_: Auto mode

## Growing a map

**Brainstorm**:
Growing a map one kept ghost at a time. Any map can be brainstormed, however it started.
_Avoid_: Brainstorm map, brainstorm mode

**Map out**:
Growing a map by many nodes in one AI call: mapping out a topic makes a whole map, mapping out a node grows its subtree.
_Avoid_: Build, knowledge graph, generate, outline (the Outline is the left panel), expand (that is unfolding)

**Graph**:
The canvas view of a map, laid out as a force graph. Never the map itself.
_Avoid_: Graph for the map itself

## Context for the AI

**Interview**:
The questions the AI asks before a map starts, to learn the user's goal, level, angle and scope.
_Avoid_: Grill, questionnaire

**Brief**:
The interview's questions and answers, kept with the map and sent with every request about it.
_Avoid_: Context, profile

**Writing guide**:
A short set of style rules the user turns on so that prose replies follow them.
_Avoid_: Skill, style, persona

**Source**:
A link in a note that a reader can follow to check it.
_Avoid_: Citation, reference

## The app

**Start screen**:
What an empty tab shows: the box for a new map's topic, ways to start it, and recent maps.
_Avoid_: Landing page, starter page, home

**Landing page**:
The website that presents Mind-Me and offers the downloads. Not part of the app.
_Avoid_: Start screen, homepage

## Look and feel

**Type scale**:
The fixed set of text sizes the whole app uses, from title down to hint, and for nodes by depth. The same in every theme.
_Avoid_: Font sizes, text size (that is the global size setting)

**Theme**:
A named set of colors for the whole app. It holds no fonts and no sizes.
_Avoid_: Skin, color scheme, mode (dark and light are themes)

**Font**:
The typeface the user picks from the fonts installed on their system, one for text and one for code. Separate from the theme.
_Avoid_: Font family per theme
