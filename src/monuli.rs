use rand::seq::SliceRandom;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gloo_storage::{errors::StorageError, LocalStorage, Storage};
use serde::{Deserialize, Serialize};

use crate::game::{
    self, KnownCounts, KnownStates, Board, Game, DEFAULT_ALLOW_PROFANITIES, DEFAULT_WORD_LENGTH,
    SUCCESS_EMOJIS,
};
use crate::manager::{
    CharacterCount, CharacterState, GameMode, KeyState, Theme, TileState, WordList, WordLists,
};

/// One cell in a compact row (SPEC 1.1). Each tile is either green, one or more yellows/browns, or empty.
#[derive(Clone, Debug, PartialEq)]
pub enum CompactTile {
    Empty,
    Correct(char),
    Yellow(char),
    /// SPEC 1.3: brown tile means that a letter would appear in more yellow tiles than is possible
    Brown(char),
    /// many yellow or brown letters in this cell; bool = is_brown
    Multi(HashSet<char>, HashSet<char>) // yellows, browns
}

/// SPEC 1.1: Compact one-row summary for a word
/// Built from guesses only (no knowledge of the correct word).
/// Greens at correct positions; yellows shown where they appeared.
/// SPEC 1.2: yellows displaced by greens go to an extra cell if there are no other yellow tiles for that letter.
/// SPEC 1.3: A brown tile means that letter is NOT in that tile, so it's same as yellow except maybe it's not elsewhere either.
pub fn compact_row(guesses: &[Vec<(char, TileState)>]) -> (Vec<CompactTile>, Vec<char>) {
    // avoid taking word_length as argument because it can be simply inferred from guesses
    assert!(!guesses.is_empty(), "compact_row: can't be called with no guesses");
    let word_length = guesses[0].len();
    assert!(guesses.iter().all(|r| r.len() == word_length), "compact_row: all guesses must be the same length");

    // find out solved positions (green) first, because they exclude other letters in that position
    let mut correct_at: Vec<Option<char>> = vec![None; word_length];
    let mut seen_count_of: HashMap<char, CharacterCount> = HashMap::new();
    for row in guesses.iter() {
        let mut seen_as_absent: HashSet<char> = HashSet::new();
        let mut current_guess_count_of: HashMap<char, usize> = HashMap::new();
        for (i, &(c, state)) in row.iter().enumerate() {
            if state == TileState::Correct {
                correct_at[i] = Some(c);
            }
            if state == TileState::Correct || state == TileState::Present {
                *current_guess_count_of.entry(c).or_insert(0) += 1;
            }
            if state == TileState::Absent {
                seen_as_absent.insert(c);
            }
        }
        for (c, count) in current_guess_count_of {
            let entry = seen_count_of.entry(c).or_insert(CharacterCount::AtLeast(0));
            match *entry {
                CharacterCount::Exactly(_) => continue,
                CharacterCount::AtLeast(old_count) => {
                    *entry = if seen_as_absent.contains(&c) {
                        CharacterCount::Exactly(count)
                    } else {
                        CharacterCount::AtLeast(old_count.max(count))
                    }
                }
            }
        }
    }
    let solved_count = correct_at.iter().filter(|g| g.is_some()).count();

    // collect tested letters in the remaining non-solved positions
    let mut is_wrong: HashMap<char, Vec<bool>> = HashMap::new();
    for row in guesses.iter() {
        for (i, &(c, state)) in row.iter().enumerate() {
            if correct_at[i].is_some() { continue; }
            if state == TileState::Present || state == TileState::Absent {
                is_wrong.entry(c).or_insert(vec![false; word_length])[i] = true;
            }
        }
    }

    // resolve yellows and browns
    let mut yellows_at: Vec<HashSet<char>> = vec![HashSet::new(); word_length];
    let mut browns_at: Vec<HashSet<char>> = vec![HashSet::new(); word_length];
    let mut extras = Vec::new();
    for (&c, &n_seen) in seen_count_of.iter() {
        let correct_count = correct_at.iter().filter(|g| **g == Some(c)).count();
        if let CharacterCount::Exactly(n_exact) = n_seen {
            assert!(correct_count <= n_exact, "there can't be more greens than exact count");
            // if greens already account for all occurrences of the letter, no yellows/browns needed
            if correct_count == n_exact { continue; }
        }
        let tried_wrong_count = match is_wrong.get(&c) {
            None => 0,
            Some(v) => v.iter().filter(|&&t| t).count(),
        };
        // if there's no more unknown cells to try, no yellows/browns needed
        //   not even extra, because the point of extra is still to be able to place it on an unknown tile
        if solved_count + tried_wrong_count == word_length { continue; }

        // allocate the yellows to tried&wrong cells first, then extra cell; leftover tried cells become brown
        let seen_count = match n_seen {
            CharacterCount::Exactly(n) => n,
            CharacterCount::AtLeast(n) => n,
        };
        let mut yellows_left = seen_count.saturating_sub(correct_count);
        if tried_wrong_count > 0 {
            let is_wrong_at = is_wrong.get(&c).unwrap();
            for i in 0..word_length {
                if is_wrong_at[i] {
                    if yellows_left > 0 {
                        yellows_at[i].insert(c); // add yellow
                        yellows_left -= 1;
                    } else {
                        browns_at[i].insert(c);  // add brown
                    }
                }
            }
        }
        while yellows_left > 0 {
            extras.push(c);                      // add yellow to extra cell
            yellows_left -= 1;
        }
    }

    let result_row: Vec<CompactTile> = (0..word_length).map(|i| {
        if let Some(c) = correct_at[i] {
            CompactTile::Correct(c)
        } else if yellows_at[i].is_empty() && browns_at[i].is_empty() {
            CompactTile::Empty
        } else if yellows_at[i].len() == 1 && browns_at[i].is_empty() {
            CompactTile::Yellow(*yellows_at[i].iter().next().unwrap())
        } else if yellows_at[i].is_empty() && browns_at[i].len() == 1 {
            CompactTile::Brown(*browns_at[i].iter().next().unwrap())
        } else {
            CompactTile::Multi(yellows_at[i].clone(), browns_at[i].clone())
        }
    }).collect();

    (result_row, extras)
}

fn get_random_word_excluding(
    word_list: WordList,
    word_length: usize,
    allow_profanities: bool,
    word_lists: &Rc<WordLists>,
    exclude: &HashSet<Vec<char>>,
) -> Option<Vec<char>> {
    let mut words = word_lists
        .get(&(word_list, word_length))?
        .iter()
        .filter(|w| !exclude.contains(*w))
        .collect::<Vec<_>>();
    if words.is_empty() {
        return None;
    }
    if !allow_profanities {
        if let Some(profanities) = word_lists.get(&(WordList::Profanities, word_length)) {
            words.retain(|word| !profanities.contains(*word));
        }
        if words.is_empty() {
            return None;
        }
    }
    let chosen = words.choose(&mut rand::thread_rng()).unwrap();
    Some((*chosen).clone())
}

/// Per-word state: same structure as Sanuli for one word (guesses, known_states, known_counts).
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct MonuliWordState {
    pub word: Vec<char>,
    pub guesses: Vec<Vec<(char, TileState)>>,
    pub known_states: Vec<KnownStates>,
    pub known_counts: Vec<KnownCounts>,
    /// Guess index at which this word was solved (None if unsolved).
    #[serde(default)]
    pub solved_at: Option<usize>,
}

impl MonuliWordState {
    fn is_solved(&self) -> bool {
        self.solved_at.is_some()
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Monuli {
    game_mode: GameMode,
    word_list: WordList,
    word_length: usize,
    n_words: usize,
    words: Vec<MonuliWordState>,
    current_guess: usize,
    streak: usize,
    message: String,

    #[serde(skip)]
    allow_profanities: bool,
    #[serde(skip)]
    word_lists: Rc<WordLists>,
    /// When Some(i), sanuli view for word i; input applies only to that word. None = list view.
    #[serde(skip)]
    pub selected_word_index: Option<usize>,
    /// When true and selected_word_index is None, show zoomed-out overview instead of list view.
    #[serde(skip)]
    pub show_overview: bool,
    /// Cursor position in the list view (index into word_order()). None = nothing selected.
    #[serde(skip)]
    pub list_cursor: Option<usize>,
    /// After switching from overview to list, scroll to center this word_order index. Consumed after render.
    #[serde(skip)]
    pub list_scroll_to: Option<usize>,
    /// After keyboard cursor move, ensure this word_order index is visible with margins. Consumed after render.
    #[serde(skip)]
    pub list_ensure_visible: Option<usize>,
    /// When true, unsolved words in word_order are sorted by compact_row quality after each guess.
    #[serde(default = "default_auto_sort")]
    pub auto_sort: bool,
}

fn default_auto_sort() -> bool {
    true
}

impl Default for Monuli {
    fn default() -> Self {
        Self::new(
            WordList::default(),
            DEFAULT_WORD_LENGTH,
            10,
            DEFAULT_ALLOW_PROFANITIES,
            Rc::new(HashMap::new()),
        )
    }
}

impl Monuli {
    pub fn max_guesses(&self) -> usize {
        self.n_words + 1
    }

    pub fn word_is_solved(&self, word_index: usize) -> bool {
        self.words
            .get(word_index)
            .map(|w| w.is_solved())
            .unwrap_or(false)
    }

    /// Sorting metric for a compact row: higher = better progress.
    /// green_count * 1_000_000 + green_pos_metric * 1_000 + yellow_count * 10 + brown_count
    fn compact_row_sort_key(&self, word_index: usize) -> u64 {
        let (cells, extra) = self.compact_row(word_index);
        let mut green_count: usize = 0;
        let mut green_pos_metric: usize = 0;
        let mut yellow_count: usize = 0;
        let mut brown_count: usize = 0;
        let n = cells.len();
        for (pos, cell) in cells.iter().enumerate() {
            match cell {
                CompactTile::Correct(_) => {
                    green_count += 1;
                    green_pos_metric += 1 << (n - 1 - pos);
                }
                CompactTile::Yellow(_) => yellow_count += 1,
                CompactTile::Brown(_) => brown_count += 1,
                CompactTile::Multi(ys, bs) => {
                    yellow_count += ys.len();
                    brown_count += bs.len();
                }
                CompactTile::Empty => {}
            }
        }
        yellow_count += extra.len();
        (green_count as u64) * 1_000_000 + (green_pos_metric as u64) * 1_000 + (yellow_count as u64) * 10 + (brown_count as u64)
    }

    /// Order for list view: unsolved first (optionally sorted by progress), then solved in solve order.
    pub fn word_order(&self) -> Vec<usize> {
        let mut unsolved: Vec<usize> = self
            .words
            .iter()
            .enumerate()
            .filter(|(_, w)| !w.is_solved())
            .map(|(i, _)| i)
            .collect();
        if self.auto_sort && self.current_guess > 0 {
            unsolved.sort_by(|&a, &b| {
                self.compact_row_sort_key(b).cmp(&self.compact_row_sort_key(a))
            });
        }
        let mut solved: Vec<(usize, usize)> = self
            .words
            .iter()
            .enumerate()
            .filter_map(|(i, w)| w.solved_at.map(|s| (i, s)))
            .collect();
        solved.sort_by_key(|&(_, s)| s);
        unsolved.extend(solved.into_iter().map(|(i, _)| i));
        unsolved
    }

    /// Compact row for one word (for monuli list view). SPEC 1.1 + 1.2.
    pub fn compact_row(&self, word_index: usize) -> (Vec<CompactTile>, Vec<char>) {
        match self.words.get(word_index) {
            Some(w) => {
                let up_to = w.solved_at.map(|s| s + 1).unwrap_or(self.current_guess);
                let submitted: Vec<_> = w
                    .guesses
                    .iter()
                    .take(up_to)
                    .filter(|row| row.len() == self.word_length)
                    .cloned()
                    .collect();
                if submitted.is_empty() {
                    return (vec![CompactTile::Empty; self.word_length], vec![]);
                }
                compact_row(&submitted)
            }
            None => (vec![CompactTile::Empty; self.word_length], vec![]),
        }
    }

    fn is_guessing(&self) -> bool {
        self.current_guess < self.max_guesses()
            && !self.words.iter().all(|w| w.is_solved())
    }

    fn is_winner(&self) -> bool {
        self.words.iter().all(|w| w.is_solved())
    }

    fn current_guess_letters(&self) -> Vec<char> {
        let idx = self
            .selected_word_index
            .filter(|&i| i < self.words.len())
            .or_else(|| self.words.iter().position(|w| !w.is_solved()))
            .unwrap_or(0);
        self.words
            .get(idx)
            .and_then(|w| w.guesses.get(self.current_guess))
            .map(|row| row.iter().map(|(c, _)| *c).collect())
            .unwrap_or_default()
    }

    fn is_guess_accepted_word(&self) -> bool {
        let letters = self.current_guess_letters();
        if letters.len() != self.word_length {
            return false;
        }
        match self.word_lists.get(&(WordList::Full, self.word_length)) {
            Some(list) => list.contains(&letters),
            None => false,
        }
    }

    /// Replace a word with another random word not in the current set (for first-guess rule).
    fn replace_word(&mut self, word_index: usize) {
        let current_set: HashSet<Vec<char>> = self.words.iter().map(|w| w.word.clone()).collect();
        if let Some(new_word) = get_random_word_excluding(
            self.word_list,
            self.word_length,
            self.allow_profanities,
            &self.word_lists,
            &current_set,
        ) {
            self.words[word_index].word = new_word;
        }
    }

    pub fn new(
        word_list: WordList,
        word_length: usize,
        n_words: usize,
        allow_profanities: bool,
        word_lists: Rc<WordLists>,
    ) -> Self {
        let mut used = HashSet::new();
        let words: Vec<Vec<char>> = (0..n_words)
            .map(|_| {
                let word = get_random_word_excluding(
                    word_list,
                    word_length,
                    allow_profanities,
                    &word_lists,
                    &used,
                )
                .unwrap_or_else(|| vec!['X'; word_length]);
                used.insert(word.clone());
                word
            })
            .collect();
        let mut m = Self::new_with_words(word_length, words, word_list, word_lists);
        m.allow_profanities = allow_profanities;
        m
    }

    /// Create a Monuli with fixed words. In production, called by `new`.
    /// In tests, caller must provide word_lists that include any guess words
    /// so submit_guess accepts them.
    pub fn new_with_words(
        word_length: usize,
        words: Vec<Vec<char>>,
        word_list: WordList,
        word_lists: Rc<WordLists>,
    ) -> Self {
        let n_words = words.len();
        let max_guesses = n_words + 1;
        let words_state: Vec<MonuliWordState> = words
            .into_iter()
            .map(|word| {
                let guesses = std::iter::repeat(Vec::with_capacity(word_length))
                    .take(max_guesses)
                    .collect::<Vec<_>>();
                let known_states = std::iter::repeat(HashMap::new())
                    .take(max_guesses)
                    .collect::<Vec<_>>();
                let known_counts = std::iter::repeat(HashMap::new())
                    .take(max_guesses)
                    .collect::<Vec<_>>();
                MonuliWordState {
                    word,
                    guesses,
                    known_states,
                    known_counts,
                    solved_at: None,
                }
            })
            .collect();
        Self {
            game_mode: GameMode::Monuli(n_words),
            word_list,
            word_length,
            n_words,
            words: words_state,
            current_guess: 0,
            streak: 0,
            message: String::new(),
            allow_profanities: true,
            word_lists,
            selected_word_index: None,
            show_overview: n_words > 20,
            list_cursor: None,
            list_scroll_to: None,
            list_ensure_visible: None,
            auto_sort: true,
        }
    }

    pub fn new_or_rehydrate(
        n_words: usize,
        word_list: WordList,
        word_length: usize,
        allow_profanities: bool,
        word_lists: Rc<WordLists>,
    ) -> Self {
        if let Ok(game) = Self::rehydrate(n_words, word_list, word_length, allow_profanities, word_lists.clone()) {
            game
        } else {
            Self::new(word_list, word_length, n_words, allow_profanities, word_lists)
        }
    }

    fn rehydrate(
        n_words: usize,
        word_list: WordList,
        word_length: usize,
        allow_profanities: bool,
        word_lists: Rc<WordLists>,
    ) -> Result<Self, StorageError> {
        let game_key = format!(
            "game|{}|{}|{}",
            serde_json::to_string(&GameMode::Monuli(n_words)).unwrap(),
            serde_json::to_string(&word_list).unwrap(),
            word_length
        );
        let mut game: Self = LocalStorage::get(&game_key)?;
        game.allow_profanities = allow_profanities;
        game.word_lists = word_lists;
        game.selected_word_index = None;
        game.show_overview = game.n_words > 20;
        game.list_cursor = None;
        game.refresh();
        Ok(game)
    }

    fn clear_message(&mut self) {
        self.message = String::new();
    }

    fn set_game_end_message(&mut self) {
        if self.is_winner() {
            self.message = format!(
                "Löysit monulit! {}",
                SUCCESS_EMOJIS.choose(&mut rand::thread_rng()).unwrap()
            );
        } else {
            let unsolved: Vec<String> = self
                .words
                .iter()
                .filter(|w| !w.is_solved())
                .map(|w| w.word.iter().collect())
                .collect();
            self.message = format!("Löytämättä jäi: \"{}\"", unsolved.join("\", \""));
        }
    }

}

impl Game for Monuli {
    fn game_mode(&self) -> &GameMode {
        &self.game_mode
    }
    fn word_list(&self) -> &WordList {
        &self.word_list
    }
    fn word_length(&self) -> usize {
        self.word_length
    }
    fn max_guesses(&self) -> usize {
        self.max_guesses()
    }
    fn boards(&self) -> Vec<Board> {
        Vec::new()
    }
    fn word(&self) -> Vec<char> {
        Vec::new()
    }
    fn last_guess(&self) -> String {
        self.current_guess_letters().into_iter().collect()
    }
    fn streak(&self) -> usize {
        self.streak
    }
    fn is_guessing(&self) -> bool {
        self.is_guessing()
    }
    fn is_reset(&self) -> bool {
        false
    }
    fn is_hidden(&self) -> bool {
        false
    }
    fn is_winner(&self) -> bool {
        self.is_winner()
    }
    fn is_unknown(&self) -> bool {
        false
    }
    fn message(&self) -> String {
        self.message.clone()
    }
    fn previous_guesses(&self) -> Vec<Vec<(char, TileState)>> {
        Vec::new()
    }
    fn set_allow_profanities(&mut self, is_allowed: bool) {
        self.allow_profanities = is_allowed;
    }
    fn title(&self) -> String {
        if self.streak > 0 {
            format!("Monuli — Putki: {}", self.streak)
        } else {
            format!("Monuli ({})", self.n_words)
        }
    }
    fn next_word(&mut self) {
        *self = Self::new(
            self.word_list,
            self.word_length,
            self.n_words,
            self.allow_profanities,
            self.word_lists.clone(),
        );
        let _ = self.persist();
    }
    fn keyboard_tilestate(&self, key: &char) -> KeyState {
        if self.selected_word_index.is_some() {
            return KeyState::Single(TileState::Unknown);
        }
        // List view: used = light blue, absent from all unsolved = black.
        let used: HashSet<char> = self
            .words
            .iter()
            .flat_map(|w| {
                w.guesses
                    .iter()
                    .take(self.current_guess + 1)
                    .flat_map(|row| row.iter().map(|(c, _)| *c))
            })
            .collect();
        if used.contains(key) {
            let unsolved = self
                .words
                .iter()
                .filter(|w| !w.is_solved())
                .collect::<Vec<_>>();
            if unsolved.is_empty() {
                return KeyState::Single(TileState::Used);
            }
            let absent_from_all = unsolved.iter().all(|w| {
                w.known_counts
                    .get(self.current_guess)
                    .and_then(|m| m.get(key))
                    == Some(&CharacterCount::Exactly(0))
            });
            return KeyState::Single(if absent_from_all {
                TileState::Absent
            } else {
                TileState::Used
            });
        }
        KeyState::Single(TileState::Unknown)
    }
    fn monuli_selected_word(&self) -> Option<usize> {
        self.selected_word_index
    }
    fn monuli_word_is_solved(&self, word_index: usize) -> bool {
        self.word_is_solved(word_index)
    }
    fn set_monuli_selected_word(&mut self, index: Option<usize>) {
        self.selected_word_index = index;
        if index.is_some() {
            self.show_overview = false;
        }
    }
    fn board_for_word(&self, word_index: usize) -> Option<Board> {
        let w = self.words.get(word_index)?;
        if let Some(solved_at) = w.solved_at {
            let guesses = w.guesses[..=solved_at].to_vec();
            Some(Board {
                guesses,
                current_guess: solved_at + 1,
                is_guessing: false,
            })
        } else {
            Some(Board {
                guesses: w.guesses.clone(),
                current_guess: self.current_guess,
                is_guessing: self.is_guessing(),
            })
        }
    }
    fn keyboard_tilestate_for_word(&self, word_index: usize, key: &char) -> KeyState {
        match self.words.get(word_index) {
            Some(w) => {
                let idx = self.current_guess.min(w.known_states.len() - 1);
                KeyState::Single(game::keyboard_tile_state(
                    key,
                    idx,
                    &w.known_states,
                    &w.known_counts,
                ))
            }
            None => KeyState::Single(TileState::Unknown),
        }
    }
    fn submit_guess(&mut self) {
        if self.current_guess_letters().len() != self.word_length {
            self.message = "Liian vähän kirjaimia!".to_owned();
            return;
        }
        if !self.is_guess_accepted_word() {
            self.message = "Ei sanulistalla.".to_owned();
            return;
        }
        self.clear_message();

        let max_guesses = self.max_guesses();
        let guess_letters: Vec<char> = self.current_guess_letters();

        // When in sanuli view, only the selected word has the current row; copy it to all unsolved words for evaluation.
        if let Some(src) = self.selected_word_index.filter(|&i| i < self.words.len()) {
            let row = self.words[src].guesses[self.current_guess].clone();
            for w in self.words.iter_mut() {
                if w.is_solved() {
                    continue;
                }
                if w.guesses[self.current_guess].len() != self.word_length {
                    w.guesses[self.current_guess] = row.clone();
                }
            }
        }

        // First guess rule: if this guess would solve any word, replace that word.
        if self.current_guess == 0 {
            let to_replace: Vec<usize> = self
                .words
                .iter()
                .enumerate()
                .filter(|(_, w)| !w.is_solved() && w.word == guess_letters)
                .map(|(i, _)| i)
                .collect();
            for i in to_replace {
                self.replace_word(i);
            }
        }

        let cursor_word_idx = self.list_cursor
            .and_then(|pos| self.word_order().get(pos).copied());

        let guess_idx = self.current_guess;
        for w in self.words.iter_mut() {
            if w.is_solved() {
                continue;
            }
            if w.guesses[guess_idx].len() == self.word_length {
                game::update_known_information(
                    &mut w.known_states,
                    &mut w.known_counts,
                    &mut w.guesses[guess_idx],
                    guess_idx,
                    &w.word,
                    max_guesses,
                );
                let all_correct = (0..w.word.len()).all(|i| {
                    w.known_states[guess_idx].get(&(w.word[i], i))
                        == Some(&CharacterState::Correct)
                });
                if all_correct {
                    w.solved_at = Some(guess_idx);
                }
            }
        }

        if let Some(wi) = cursor_word_idx {
            if self.word_is_solved(wi) {
                self.list_cursor = None;
            }
        }

        self.current_guess += 1;
        if !self.is_guessing() {
            self.set_game_end_message();
            if self.is_winner() {
                self.streak += 1;
            } else {
                self.streak = 0;
            }
        }
        let _ = self.persist();
    }
    fn push_character(&mut self, character: char) {
        if !self.is_guessing() {
            return;
        }
        self.clear_message();
        let idx = self.current_guess;
        let words_to_update: Vec<usize> = match self.selected_word_index {
            Some(i) if i < self.words.len() => vec![i],
            _ => (0..self.words.len()).collect(),
        };
        for i in words_to_update {
            let w = &mut self.words[i];
            if w.is_solved() {
                continue;
            }
            if w.guesses[idx].len() < self.word_length {
                let tile_state = game::hint_tile_state(
                    character,
                    w.guesses[idx].len(),
                    idx,
                    &w.known_states,
                    &w.known_counts,
                );
                w.guesses[idx].push((character, tile_state));
            }
        }
    }
    fn pop_character(&mut self) {
        if !self.is_guessing() {
            return;
        }
        self.clear_message();
        let idx = self.current_guess;
        let words_to_update: Vec<usize> = match self.selected_word_index {
            Some(i) if i < self.words.len() => vec![i],
            _ => (0..self.words.len()).collect(),
        };
        for i in words_to_update {
            if self.words[i].is_solved() {
                continue;
            }
            if !self.words[i].guesses[idx].is_empty() {
                self.words[i].guesses[idx].pop();
            }
        }
    }
    fn share_emojis(&self, _theme: Theme) -> Option<String> {
        None
    }
    fn share_link(&self) -> Option<String> {
        None
    }
    fn reveal_hidden_tiles(&mut self) {}
    fn reset(&mut self) {
        *self = Self::new(
            self.word_list,
            self.word_length,
            self.n_words,
            self.allow_profanities,
            self.word_lists.clone(),
        );
    }
    fn refresh(&mut self) {
        let max_guesses = self.max_guesses();
        for w in self.words.iter_mut() {
            w.known_states = std::iter::repeat(HashMap::new())
                .take(max_guesses)
                .collect::<Vec<_>>();
            w.known_counts = std::iter::repeat(HashMap::new())
                .take(max_guesses)
                .collect::<Vec<_>>();
            for guess_index in 0..self.current_guess {
                if w.guesses[guess_index].len() == self.word_length {
                    game::update_known_information(
                        &mut w.known_states,
                        &mut w.known_counts,
                        &mut w.guesses[guess_index],
                        guess_index,
                        &w.word,
                        max_guesses,
                    );
                }
            }
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn persist(&self) -> Result<(), StorageError> {
        let game_key = format!(
            "game|{}|{}|{}",
            serde_json::to_string(&GameMode::Monuli(self.n_words)).unwrap(),
            serde_json::to_string(&self.word_list).unwrap(),
            self.word_length
        );
        LocalStorage::set(&game_key, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_monuli(words: &[&str]) -> Monuli {
        let word_length = words[0].len();
        let all_words: HashSet<Vec<char>> = words.iter().map(|w| w.chars().collect()).collect();
        let mut word_lists: WordLists = HashMap::new();
        word_lists.insert((WordList::Full, word_length), all_words.clone());
        word_lists.insert((WordList::Common, word_length), all_words);
        Monuli::new_with_words(
            word_length,
            words.iter().map(|w| w.chars().collect()).collect(),
            WordList::Common,
            Rc::new(word_lists),
        )
    }

    fn type_and_submit(monuli: &mut Monuli, guess: &str) {
        for c in guess.chars() {
            monuli.push_character(c);
        }
        monuli.submit_guess();
    }

    #[test]
    fn solve_all_words_one_by_one() {
        let words = ["LAHTI", "HANHI", "HURJA", "MÄÄRÄ", "LEIPÄ", "PALVI", "MÖKKI", "RAMPA"];
        let mut m = make_test_monuli(&words);
        assert_eq!(m.n_words, 8);
        assert_eq!(m.max_guesses(), 9);
        assert!(m.is_guessing());

        // First guess: a throwaway (can't solve on first guess due to replacement rule).
        // Use "LAHTI" which is in our word list; it will be replaced since it matches word 0.
        // Use a neutral word instead — add it to word list.
        // Actually, let's just add an extra word to the word list for the first guess.
        let extra: Vec<char> = "TAKKI".chars().collect();
        if let Some(list) = Rc::get_mut(&mut m.word_lists) {
            list.get_mut(&(WordList::Full, 5)).unwrap().insert(extra.clone());
            list.get_mut(&(WordList::Common, 5)).unwrap().insert(extra);
        }
        type_and_submit(&mut m, "TAKKI");
        assert_eq!(m.current_guess, 1);
        assert!(m.is_guessing());

        // Now solve each word one by one (cheating: guess the correct answer).
        // In Monuli, all unsolved words receive each guess, so guessing word[i] solves it.
        for i in 0..8 {
            assert!(m.is_guessing(), "should still be guessing before solving word {i}");
            type_and_submit(&mut m, words[i]);
            assert!(
                m.words[i].is_solved(),
                "word {i} ({}) should be solved", words[i]
            );
            // keyboard_tilestate_for_word must not panic even after the game ends
            for &c in &['A', 'B', 'C'] {
                let _ = m.keyboard_tilestate_for_word(i, &c);
            }
        }

        assert!(!m.is_guessing(), "game should be over (all solved)");
        assert!(m.is_winner());
        assert_eq!(m.current_guess, 9);

        // After game ends, keyboard_tilestate_for_word must still work
        for wi in 0..8 {
            for &c in &['A', 'L', 'H', 'T', 'I'] {
                let _ = m.keyboard_tilestate_for_word(wi, &c);
            }
        }
    }

    fn test_compact_row(word: &str, guesses: &[&str], expected: &[CompactTile], expected_extra: &[char]) {
        let w: Vec<char> = word.chars().collect();
        let max = guesses.len();
        let mut states = vec![HashMap::new(); max];
        let mut counts = vec![HashMap::new(); max];
        let mut rows: Vec<Vec<(char, TileState)>> = guesses
            .iter()
            .map(|s| s.chars().map(|c| (c, TileState::Unknown)).collect())
            .collect();
        for (i, row) in rows.iter_mut().enumerate() {
            game::update_known_information(&mut states, &mut counts, row, i, &w, max);
        }
        let (result, extra) = compact_row(&rows);
        assert_eq!(result, expected, "word={word}, guesses={guesses:?}");
        assert_eq!(extra, expected_extra, "word={word}, guesses={guesses:?} (extra cell)");
    }

    fn g(c: char) -> CompactTile { CompactTile::Correct(c) }
    fn y(c: char) -> CompactTile { CompactTile::Yellow(c) }
    fn b(c: char) -> CompactTile { CompactTile::Brown(c) }
    fn m(ys: &[char], bs: &[char]) -> CompactTile { CompactTile::Multi(ys.iter().cloned().collect(), bs.iter().cloned().collect()) }
    const E: CompactTile = CompactTile::Empty;

    #[test]
    fn compact_row_cases() {
        // brown H suppressed (KAU**H**A) when green (**H**IENO) accounts for exact count of Hs (i.e. 1, known from absent H in HU**H**TA)
        test_compact_row("HURJA", &["HIENO", "KAUHA", "HUHTA"], &[g('H'), g('U'), b('U'), E, g('A')], &[]);
        // there should be no displaced A when there's already two green A's
        test_compact_row("SALPA", &["SUOMI", "HAHMO", "KOIPI", "OIKEA", "OHJAS"], &[g('S'), g('A'), E, g('P'), g('A')], &[]);
        // SPEC 1.3.1: L appears 2x as yellow but max 1 per guess → brown
        test_compact_row("LAHTI", &["KAALI", "PALVI"], &[E, g('A'), y('L'), b('L'), g('I')], &[]);
        // SPEC 1.3: I appears 2x as yellow but max 1 per guess → brown
        test_compact_row("LEIPÄ", &["PILLI", "LAPSI"], &[g('L'), y('I'), y('P'), E, b('I')], &[]);
        // SPEC 1.2: displaced yellows go to extra cell
        test_compact_row("MÄÄRÄ", &["ÄÄLIÖ", "RAMPA", "MÖKKI"], &[g('M'), g('Ä'), b('M'), E, E], &['Ä', 'R']);
        // SPEC 1.1.1: two yellows in same cell
        test_compact_row("LAHTI", &["KAALI", "TARHA"], &[y('T'), g('A'), E, m(&['L', 'H'], &[]), g('I')], &[]);
        // one green, four yellow
        test_compact_row("HANHI", &["HIHNA"], &[g('H'), y('I'), y('H'), y('N'), y('A')], &[]);
        // duplicate yellow in same position deduped
        test_compact_row("LAHTI", &["KAALI", "MAALI"], &[E, g('A'), E, y('L'), g('I')], &[]);
        // all green
        test_compact_row("LAHTI", &["KAALI", "LAHTI"], &[g('L'), g('A'), g('H'), g('T'), g('I')], &[]);
    }
}
