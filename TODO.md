# Monuli implementation plan

High-level (from SPEC): N words, N+1 guesses total, first guess guaranteed wrong. New UI: compact one-row-per-word in monuli view; click word → sanuli view for that word. Overview/zoom when many words.

[x] Models and tests (sections 1–2)
   Implement Monuli game logic and compact-row function + unit tests. No UI yet.

[x] Integration (section 3)
   Add Monuli to menu and manager so that selecting Monuli loads the game (can still show a placeholder or simple “Monuli” message in main view).

[x] Monuli list view (section 4.1, 4.3)
   Implement list view with compact rows and keyboard (used/absent). No sanuli sub-view yet.

[x] Sanuli sub-view (section 4.2, 4.3, 4.4)
   Add selection state, sanuli view for one word, and back button.

[ ] Overview/zoom (section 5) — skipped: N=10 fits on one screen (per 5.3)
   Add overview when N is large and column/click-to-zoom.

[x] Help and polish (section 6)
   Documentation and edge cases.

**Review (SPEC 1.1 + implementation so far):** Sections 1 and 3.2 are done. **SPEC 1.1 (tarkennuksia):** In Monulinäkymä, show *where* wrong-position letters landed; the same cell can get multiple yellows from different guesses (e.g. LAHTI: KAALI then TARHA → cell 3 has L and H). When a cell has 2–4 yellows, render as 2×2 grid (quarter-size). So current `compact_row` (yellows filled left-to-right) does not match; need per-position yellows and new cell type (2.5). Section 4.1 list view must render: empty | green+letter | one yellow | 2×2 grid of yellows. Sections 3.1, 3.3–3.4, 4–6 unchanged.

---

## 1. Monuli models and core logic

- [x] **1.1** Add `GameMode::Monuli(usize)` (or similar) for N words. Decide where N is stored (e.g. fixed 10/20/50/100 or user choice) and add to manager/serialization if needed.
- [x] **1.2** Add `Monuli` game struct (similar to `Neluli`): holds N `Sanuli`-like subgames (or a dedicated per-word state). First word must be fetched “wrong” (SPEC: if first would be correct, swap it). Ensure exactly N+1 total guesses across all words.
- [x] **1.3** Per-word state: for each of the N words, track guesses and known_states/known_counts. One shared “current guess index” (0..=N) so that one typed guess is applied to all active words, then advance. When a word is solved, it no longer receives new letters; when all are solved or guesses run out, game ends.
- [x] **1.4** Implement “first guess wrong” rule: when initializing Monuli, if the first drawn word would be solved by the first user guess, replace that word with another from the list (or equivalent rule so the first guess is never fully correct).
- [x] **1.5** Implement `Game` trait for `Monuli`: `boards()` may be unused for monuli view; expose enough for “current guess”, “which words are solved”, “per-word guess history” and “per-word known_states/known_counts” for rendering and for sanuli sub-view.
- [x] **1.6** Persist/rehydrate Monuli (localStorage key by `GameMode::Monuli(n)`, word_list, word_length) and integrate with `Manager::switch_active_game` and `Manager::new` rehydration.

---

## 2. Monuli compact row (word summary) – model and tests

- [x] **2.1** Define “compact row” data for one word: from all guesses so far for that word, compute a single row of `word_length` cells: green at correct positions, then fill non-green cells left-to-right with yellow letters (from any guess), accounting for multiplicity (SPEC: if same letter guessed twice in a word, it can appear as two yellows). No letter in remaining cells (black/empty).
- [x] **2.2** Formalize rules: greens from `known_states` (Correct); yellows = letters that got Present in some guess, minus greens already placed, respecting “multiple same letter” (e.g. PASTA → two A’s: one green, one yellow if applicable). Write pure function `compact_row(word_length, guesses, known_states, known_counts) -> Vec<(Option<char>, TileState)>` (or similar).
- [x] **2.3** Unit tests for compact row (SPEC test cases):
- [x] **2.4** Solved words: `is_solved()`, `word_order()` (unsolved first, then solved). Separator before solved (SPEC: e.g. black row).
- [x] **2.5** **SPEC 1.1:** Per-position yellows: each cell can have multiple yellows (from different guesses). New compact model: e.g. `CompactCell { green: Option<char>, yellows: Vec<char> }`; update `compact_row` so UI can render 2×2 when `yellows.len() > 1`. Test: LAHTI + KAALI + TARHA → cell 3 has [L, H].

---

## 3. Monuli mode in menu and manager

- [x] **3.1** Add Monuli to `GameMode` and to menu modal (e.g. “Monuli” button; optionally sub-options for N or use fixed N).
- [x] **3.2** In `Manager::new` and `switch_active_game`, handle `GameMode::Monuli`: create/rehydrate `Monuli` and set `game = Some(Box::new(monuli))`.
- [x] **3.3** Wire `change_game_mode` and persistence so Monuli is saved/restored like Neluli (word_list, word_length, allow_profanities).
- [x] **3.4** Hide or adapt “word length / word list” in menu for Monuli if needed (SPEC doesn’t require different list; keep same as other modes or add simple option).

---

## 4. Monuli UI – two views

- [x] **4.1** **Monulinäkymä (list view)**
  - Unlike neluli, one row for the current guess, it should be shown above the (scrollable) list of words
  - Between guess row and keyboard, (scrollable) list of one row per word using compact row. **SPEC 1.1:** Each cell: empty | green+letter | one yellow+letter | 2×2 grid (2–4 yellows in one cell).
  - Order: unsolved first (any internal order), then separator (e.g. black row), then solved in solve order.
  - Keyboard: not colored by hit; color by “used” (e.g. light blue). If a letter is known absent from all remaining words, can use black (reuse sanuli absent style).
  - Implement later: Scrollable list can be multiple columns on wide screens. For now, only one column.
  - Click on a word row → open “sanuli view” for that word (see 4.2).
- [x] **4.2** **Sanulinäkymä (single-word view)**
  - When a word is selected, show classic sanuli board for that word only: all previous guesses as rows, current guess row, full tile coloring.
  - Keyboard colored by that word’s known_states/known_counts (same as current Sanuli).
  - “Back” button to return to monuli list view (no new guess consumed).
- [x] **4.3** App-level routing: when `GameMode::Monuli` and no word selected → render monuli list view; when word selected → render sanuli view for that word. Manager/state must hold “selected word index” (or None).
- [x] **4.4** Enter/Backspace/typing: in list view, typing applies to “current guess” and submits to all active words (same as Neluli-style); in sanuli view, typing applies only to the selected word. On submit in sanuli view, same rule: one guess consumed globally. Clarify: back from sanuli view doesn’t change guess state; next keypress still applies to the same “current” guess.

---

## 5. Overview / zoom (many words)

- [ ] **5.1** If word count is large (e.g. > 10–15), show “overview” first: very compact rows (colors only, no letters), possibly multiple columns (e.g. 4 columns of N/4 rows). Scroll horizontally if needed.
- [ ] **5.2** On overview, click a column/region → zoom into “monuli list view” for that slice (scroll position so that region is visible). Then list view scroll and click-to-sanuli as in 4.1–4.2.
- [ ] **5.3** Optional: when words fit on one screen (e.g. N ≤ 10), skip overview and show list view directly (no 2-step zoom).

---

## 6. Help and polish

- [x] **6.1** Add Monuli to help modal text (rules: N words, N+1 guesses, first wrong, one row per word, click for detail).
- [x] **6.2** Share / stats: decide if Monuli has share-emojis or share-link; if not, disable or show “Ei saatavilla” for Monuli. Same for streak/totals if needed.
- [x] **6.3** Test on narrow (portrait) and wide (landscape) layouts; ensure scroll and column layout work.

---

