# Monuli implementation plan

High-level (from SPEC): N words, N+1 guesses total, first guess guaranteed wrong. New UI: compact one-row-per-word in monuli view; click word → sanuli view for that word. Overview/zoom when many words.

[ ] Models and tests (sections 1–2)
   Implement Monuli game logic and compact-row function + unit tests. No UI yet.

[ ] Integration (section 3)
   Add Monuli to menu and manager so that selecting Monuli loads the game (can still show a placeholder or simple “Monuli” message in main view).

[ ] Monuli list view (section 4.1, 4.3)
   Implement list view with compact rows and keyboard (used/absent). No sanuli sub-view yet.

[ ] Sanuli sub-view (section 4.2, 4.3, 4.4)
   Add selection state, sanuli view for one word, and back button.

[ ] Overview/zoom (section 5)
   Add overview when N is large and column/click-to-zoom.

[ ] Help and polish (section 6)
   Documentation and edge cases.

---

## 1. Monuli models and core logic

- [ ] **1.1** Add `GameMode::Monuli(usize)` (or similar) for N words. Decide where N is stored (e.g. fixed 10/20/50/100 or user choice) and add to manager/serialization if needed.
- [ ] **1.2** Add `Monuli` game struct (similar to `Neluli`): holds N `Sanuli`-like subgames (or a dedicated per-word state). First word must be fetched “wrong” (SPEC: if first would be correct, swap it). Ensure exactly N+1 total guesses across all words.
- [ ] **1.3** Per-word state: for each of the N words, track guesses and known_states/known_counts. One shared “current guess index” (0..=N) so that one typed guess is applied to all active words, then advance. When a word is solved, it no longer receives new letters; when all are solved or guesses run out, game ends.
- [ ] **1.4** Implement “first guess wrong” rule: when initializing Monuli, if the first drawn word would be solved by the first user guess, replace that word with another from the list (or equivalent rule so the first guess is never fully correct).
- [ ] **1.5** Implement `Game` trait for `Monuli`: `boards()` may be unused for monuli view; expose enough for “current guess”, “which words are solved”, “per-word guess history” and “per-word known_states/known_counts” for rendering and for sanuli sub-view.
- [ ] **1.6** Persist/rehydrate Monuli (localStorage key by `GameMode::Monuli(n)`, word_list, word_length) and integrate with `Manager::switch_active_game` and `Manager::new` rehydration.

---

## 2. Monuli compact row (word summary) – model and tests

- [ ] **2.1** Define “compact row” data for one word: from all guesses so far for that word, compute a single row of `word_length` cells: green at correct positions, then fill non-green cells left-to-right with yellow letters (from any guess), accounting for multiplicity (SPEC: if same letter guessed twice in a word, it can appear as two yellows). No letter in remaining cells (black/empty).
- [ ] **2.2** Formalize rules: greens from `known_states` (Correct); yellows = letters that got Present in some guess, minus greens already placed, respecting “multiple same letter” (e.g. PASTA → two A’s: one green, one yellow if applicable). Write pure function `compact_row(word_length, guesses, known_states, known_counts) -> Vec<(Option<char>, TileState)>` (or similar).
- [ ] **2.3** Unit tests for compact row (SPEC test cases):
  - Greens `.A..I`, yellows `A.L..`: e.g. one green A, one green I; one yellow A, one yellow L; rest empty. Case: letter tried once vs twice (e.g. PASTA) and effect on showing yellow A.
  - Edge: all 5 green (solved) in one row.
  - Edge: 4 green + 1 yellow for last position (same letter found but wrong position in that guess).
- [ ] **2.4** Decide representation for “solved” words (e.g. flag or full green row) and “solved order” for ordering in list (solved words at bottom, in solve order; optional separator row before solved section).

---

## 3. Monuli mode in menu and manager

- [ ] **3.1** Add Monuli to `GameMode` and to menu modal (e.g. “Monuli” button; optionally sub-options for N or use fixed N).
- [ ] **3.2** In `Manager::new` and `switch_active_game`, handle `GameMode::Monuli`: create/rehydrate `Monuli` and set `game = Some(Box::new(monuli))`.
- [ ] **3.3** Wire `change_game_mode` and persistence so Monuli is saved/restored like Neluli (word_list, word_length, allow_profanities).
- [ ] **3.4** Hide or adapt “word length / word list” in menu for Monuli if needed (SPEC doesn’t require different list; keep same as other modes or add simple option).

---

## 4. Monuli UI – two views

- [ ] **4.1** **Monulinäkymä (list view)**
  - Unlike neluli, one row for the current guess, it should be shown above the (scrollable) list of words
  - Between guess row and keyboard, (scrollable) list of one row per word using compact row (green/yellow/empty).
  - Order: unsolved first (any internal order), then separator (e.g. black row), then solved in solve order.
  - Keyboard: not colored by hit; color by “used” (e.g. light blue). If a letter is known absent from all remaining words, can use black (reuse sanuli absent style).
  - Implement later: Scrollable list can be multiple columns on wide screens. For now, only one column.
  - Click on a word row → open “sanuli view” for that word (see 4.2).
- [ ] **4.2** **Sanulinäkymä (single-word view)**
  - When a word is selected, show classic sanuli board for that word only: all previous guesses as rows, current guess row, full tile coloring.
  - Keyboard colored by that word’s known_states/known_counts (same as current Sanuli).
  - “Back” button to return to monuli list view (no new guess consumed).
- [ ] **4.3** App-level routing: when `GameMode::Monuli` and no word selected → render monuli list view; when word selected → render sanuli view for that word. Manager/state must hold “selected word index” (or None).
- [ ] **4.4** Enter/Backspace/typing: in list view, typing applies to “current guess” and submits to all active words (same as Neluli-style); in sanuli view, typing applies only to the selected word. On submit in sanuli view, same rule: one guess consumed globally. Clarify: back from sanuli view doesn’t change guess state; next keypress still applies to the same “current” guess.

---

## 5. Overview / zoom (many words)

- [ ] **5.1** If word count is large (e.g. > 10–15), show “overview” first: very compact rows (colors only, no letters), possibly multiple columns (e.g. 4 columns of N/4 rows). Scroll horizontally if needed.
- [ ] **5.2** On overview, click a column/region → zoom into “monuli list view” for that slice (scroll position so that region is visible). Then list view scroll and click-to-sanuli as in 4.1–4.2.
- [ ] **5.3** Optional: when words fit on one screen (e.g. N ≤ 10), skip overview and show list view directly (no 2-step zoom).

---

## 6. Help and polish

- [ ] **6.1** Add Monuli to help modal text (rules: N words, N+1 guesses, first wrong, one row per word, click for detail).
- [ ] **6.2** Share / stats: decide if Monuli has share-emojis or share-link; if not, disable or show “Ei saatavilla” for Monuli. Same for streak/totals if needed.
- [ ] **6.3** Test on narrow (portrait) and wide (landscape) layouts; ensure scroll and column layout work.

---

