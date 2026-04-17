use rand::seq::SliceRandom;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gloo_storage::{errors::StorageError, LocalStorage, Storage};
use serde::{Deserialize, Serialize};

use crate::game::{self, KnownCounts, KnownStates, Board, Game};
use crate::manager::{
    CharacterCount, CharacterState, DEFAULT_ALLOW_PROFANITIES, GameMode, KeyState,
    SUCCESS_EMOJIS, TileState, WordList, WordLists, EnterButton,
};

#[cfg(web_sys_unstable_apis)]
use crate::manager::Theme;

/// Available choices for monuli size
pub const MONULI_N_WORDS: [usize; 9] = [8, 10, 16, 25, 36, 49, 64, 81, 100];

/// Number of guesses in monuli, depending on number of words
/// Currently there is a *cough* feature that when a word is all-green, it is solved;
///   this means large monuli(N) can be solved in less than N tries
pub const fn max_guesses(n_words: usize) -> usize {
    4 + n_words * 9 / 10
}

/// One cell in a compact row (SPEC 1.1). Each tile is either green, one or more yellows/browns, or empty.
#[derive(Clone, Debug, PartialEq)]
pub enum CompactTile {
    Empty,
    Correct(char),
    Present(char),
    /// SPEC 1.3: maybe-present tile means that a letter would appear in more yellow tiles than is possible
    MaybePresent(char),
    /// SPEC 1.4: absent tile means that no more of the letter can be in the word
    Absent(char),
    /// multiple non-green letters in this cell
    Multi(HashSet<char>, HashSet<char>, HashSet<char>) // presents, maybe_presents, absents
}

/// SPEC 1.1: Compact one-row summary for a word
/// Built from guesses only (no knowledge of the correct word).
/// Greens at correct positions; yellows shown where they appeared.
/// SPEC 1.2: yellows displaced by greens go to an extra cell if there are no other yellow tiles for that letter.
/// SPEC 1.3: A brown tile means that letter is NOT in that tile, so it's same as yellow except maybe it's not elsewhere either.
pub fn compact_row(guesses: &[Vec<(char, TileState)>]) -> (Vec<CompactTile>, HashSet<char>) {
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
    let mut absent_at: Vec<HashSet<char>> = vec![HashSet::new(); word_length];
    let mut extras = HashSet::new();
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

        // allocate the yellows to tried&wrong cells first, then extra cell
        // leftover tried cells become either brown (if count is `AtLeast`) or absent (if `Exactly`)
        let mut yellows_left = usize::from(n_seen).saturating_sub(correct_count);
        if tried_wrong_count > 0 {
            let is_wrong_at = is_wrong.get(&c).unwrap();
            for i in 0..word_length {
                if is_wrong_at[i] {
                    if yellows_left > 0 {
                        yellows_at[i].insert(c);
                        yellows_left -= 1;
                    } else {
                        match n_seen {
                            CharacterCount::Exactly(_) => { absent_at[i].insert(c); },
                            CharacterCount::AtLeast(_) => { browns_at[i].insert(c); },
                        }
                    }
                }
            }
        }
        while yellows_left > 0 {
            extras.insert(c);
            yellows_left -= 1;
        }
    }

    let result_row: Vec<CompactTile> = (0..word_length).map(|i| {
        if let Some(c) = correct_at[i] {
            CompactTile::Correct(c)
        } else if yellows_at[i].is_empty() && browns_at[i].is_empty() && absent_at[i].is_empty() {
            CompactTile::Empty
        } else if yellows_at[i].len() == 1 && browns_at[i].is_empty() && absent_at[i].is_empty() {
            CompactTile::Present(*yellows_at[i].iter().next().unwrap())
        } else if yellows_at[i].is_empty() && browns_at[i].len() == 1 && absent_at[i].is_empty() {
            CompactTile::MaybePresent(*browns_at[i].iter().next().unwrap())
        } else if yellows_at[i].is_empty() && browns_at[i].is_empty() && absent_at[i].len() == 1 {
            CompactTile::Absent(*absent_at[i].iter().next().unwrap())
        } else {
            CompactTile::Multi(yellows_at[i].clone(), browns_at[i].clone(), absent_at[i].clone())
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
pub struct MonuliWord {
    pub word: Vec<char>,
    pub guesses: Vec<Vec<(char, TileState)>>,
    #[serde(skip)]
    pub known_states: Vec<KnownStates>,
    #[serde(skip)]
    pub known_counts: Vec<KnownCounts>,
    /// Guess index at which this word was solved (None if unsolved).
    pub solved_at: Option<usize>,
}

impl MonuliWord {
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
    words: Vec<MonuliWord>,
    message: String,

    pub current_guess: usize,
    pub streak: usize,
    pub best_score: usize,

    #[serde(skip)]
    allow_profanities: bool,
    #[serde(skip)]
    pub word_lists: Rc<WordLists>,

    /// When Some(i), show sanuli view for word i; input applies only to that word. None = list view.
    #[serde(skip)]
    pub selected_word_index: Option<usize>,
}

impl Monuli {
    /// Move from monuli list view to sanuli view
    // pub fn select_monuli_word(&mut self, word_index: usize) {
    //     self.selected_word_index = Some(word_index);
    // }

    /// Move from sanuli view to monuli list view
    // pub fn deselect_monuli_word(&mut self) {
    //     self.selected_word_index = None;
    // }

    /// Sanuli board for individual monuli word
    pub fn board_for_word(&self, word_index: usize) -> Option<Board> {
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
                CompactTile::Present(_) => { yellow_count += 1; }
                CompactTile::MaybePresent(_) => { brown_count += 1; }
                CompactTile::Absent(_) => {},
                CompactTile::Multi(ps, mps, _as) => {
                    yellow_count += ps.len();
                    brown_count += mps.len();
                }
                CompactTile::Empty => {}
            }
        }
        yellow_count += extra.len();
        (green_count as u64) * 1_000_000 + (green_pos_metric as u64) * 1_000 + (yellow_count as u64) * 10 + (brown_count as u64)
    }

    /// Order for list view: unsolved first (optionally sorted by progress), then solved in solve order.
    /// # Returns
    /// Indices into self.words, in display order.
    pub fn word_order(&self, auto_sort: bool) -> Vec<usize> {
        let mut unsolved: Vec<usize> = self
            .words
            .iter()
            .enumerate()
            .filter(|(_, w)| !w.is_solved())
            .map(|(i, _)| i)
            .collect();
        if auto_sort && self.current_guess > 0 {
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

    pub fn is_winner(&self) -> bool {
        self.words.iter().all(|w| w.is_solved())
    }

    /// Compact row for one word (for monuli list view). SPEC 1.1 + 1.2.
    pub fn compact_row(&self, word_index: usize) -> (Vec<CompactTile>, HashSet<char>) {
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
                    return (vec![CompactTile::Empty; self.word_length], HashSet::new());
                }
                compact_row(&submitted)
            }
            None => (vec![CompactTile::Empty; self.word_length], HashSet::new()),
        }
    }

    fn current_guess_letters(&self) -> Vec<char> {
        let idx = self.words.iter().position(|w| !w.is_solved()).unwrap_or(0);
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
        let max_guesses = max_guesses(n_words);
        let words_state: Vec<MonuliWord> = words
            .into_iter()
            .map(|word| {
                let guesses = std::iter::repeat_n(Vec::with_capacity(word_length), max_guesses)
                    .collect::<Vec<_>>();
                let known_states = std::iter::repeat_n(HashMap::new(), max_guesses)
                    .collect::<Vec<_>>();
                let known_counts = std::iter::repeat_n(HashMap::new(), max_guesses)
                    .collect::<Vec<_>>();
                MonuliWord {
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
            best_score: 0,
            message: String::new(),
            selected_word_index: None,
            allow_profanities: DEFAULT_ALLOW_PROFANITIES,
            word_lists,
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
        let game_key = Self::localstorage_key(n_words, word_list, word_length);
        let mut game: Self = LocalStorage::get(&game_key)?;
        game.allow_profanities = allow_profanities;
        game.word_lists = word_lists;
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

    fn localstorage_key(n_words: usize, word_list: WordList, word_length: usize) -> String {
        format!(
            "game|monuli|{}|{}|{}",
            n_words,
            serde_json::to_string(&word_list).unwrap(),
            word_length
        )
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
        max_guesses(self.n_words)
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
        self.current_guess < self.max_guesses() && !self.is_winner()
    }
    fn is_reset(&self) -> bool {
        false
    }
    fn is_hidden(&self) -> bool {
        false
    }
    fn is_winner(&self) -> bool {
        self.words.iter().all(|w| w.is_solved())
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
        format!("{}:n monuli", self.n_words)
    }
    fn next_word(&mut self) {
        let best_score = self.best_score;
        let streak = self.streak;
        *self = Self::new(
            self.word_list,
            self.word_length,
            self.n_words,
            self.allow_profanities,
            self.word_lists.clone(),
        );
        self.best_score = best_score;
        self.streak = streak;
        let _ = self.persist();
    }
    fn keyboard_tilestate(&self, key: &char) -> KeyState {
        if let Some(word_index) = self.selected_word_index {
            match self.words.get(word_index) {
                Some(w) => {
                    let idx = self.current_guess.min(w.known_states.len() - 1);
                    return KeyState::Single(game::keyboard_tile_state(
                        key,
                        idx,
                        &w.known_states,
                        &w.known_counts,
                    ));
                }
                None => return KeyState::Single(TileState::Unknown),
            }
        } else {
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
                        .get(self.current_guess.min(w.known_counts.len() - 1))
                        .and_then(|m| m.get(key))
                        == Some(&CharacterCount::Exactly(0))
                });
                return KeyState::Single(if absent_from_all {
                    TileState::Absent
                } else {
                    TileState::Used
                });
            }
        }
        KeyState::Single(TileState::Unknown)
    }

    fn enter_button_state(&self) -> EnterButton {
        if self.is_guessing() {
            EnterButton::SubmitGuess
        } else {
            EnterButton::RestartGame
        }
    }

    fn as_monuli(&self) -> Option<&Monuli> {
        Some(self)
    }

    fn as_monuli_mut(&mut self) -> Option<&mut Monuli> {
        Some(self)
    }

    fn submit_guess(&mut self) {
        let guess_letters = self.current_guess_letters();
        if guess_letters.len() != self.word_length {
            self.message = "Liian vähän kirjaimia!".to_owned();
            return;
        }
        if !self.is_guess_accepted_word() {
            self.message = "Ei sanulistalla.".to_owned();
            return;
        }
        self.clear_message();

        let guess_idx = self.current_guess;
        for w in self.words.iter_mut() {
            if w.is_solved() {
                continue;
            }
            if w.guesses[guess_idx].len() != self.word_length {
                // If this word doesn't have a guess yet, copy from the first unsolved one
                // Wait, all words should have the same guesses pushed to them anyway.
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
                    max_guesses(self.n_words),
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

        self.current_guess += 1;
        if !self.is_guessing() {
            self.set_game_end_message();
            if self.is_winner() {
                self.streak += 1;
                if self.best_score == 0 || self.current_guess < self.best_score {
                    self.best_score = self.current_guess;
                }
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
        for i in 0..self.words.len() {
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
        for i in 0..self.words.len() {
            if self.words[i].is_solved() {
                continue;
            }
            if !self.words[i].guesses[idx].is_empty() {
                self.words[i].guesses[idx].pop();
            }
        }
    }
    #[cfg(web_sys_unstable_apis)]
    fn share_emojis(&self, _theme: Theme) -> Option<String> {
        None
    }
    #[cfg(web_sys_unstable_apis)]
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
            w.known_states = std::iter::repeat_n(HashMap::new(), max_guesses)
                .collect::<Vec<_>>();
            w.known_counts = std::iter::repeat_n(HashMap::new(), max_guesses)
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

    fn persist(&self) -> Result<(), StorageError> {
        let game_key = Self::localstorage_key(self.n_words, self.word_list, self.word_length);
        LocalStorage::set(game_key, self)
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

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
        assert_eq!(m.max_guesses(), 11);
        assert!(m.is_guessing());

        // Add a throwaway word to the word list for the first guess.
        let extra: Vec<char> = "TAKKI".chars().collect();
        if let Some(list) = Rc::get_mut(&mut m.word_lists) {
            list.get_mut(&(WordList::Full, 5)).unwrap().insert(extra.clone());
            list.get_mut(&(WordList::Common, 5)).unwrap().insert(extra);
        }
        type_and_submit(&mut m, "TAKKI");
        assert_eq!(m.current_guess, 1);
        assert!(m.is_guessing());

        for i in 0..8 {
            assert!(m.is_guessing(), "should still be guessing before solving word {i}");
            type_and_submit(&mut m, words[i]);
            assert!(
                m.words[i].is_solved(),
                "word {i} ({}) should be solved", words[i]
            );
        }

        assert!(!m.is_guessing(), "game should be over (all solved)");
        assert!(m.is_winner());
        assert_eq!(m.current_guess, 9);
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
        assert_eq!(extra.into_iter().sorted().collect::<Vec<_>>(), expected_extra, "word={word}, guesses={guesses:?} (extra cell)");
    }

    fn g(c: char) -> CompactTile { CompactTile::Correct(c) }
    fn y(c: char) -> CompactTile { CompactTile::Present(c) }
    fn b(c: char) -> CompactTile { CompactTile::MaybePresent(c) }
    fn h(c: char) -> CompactTile { CompactTile::Absent(c) }
    fn m(ps: &[char], mps: &[char], aas: &[char]) -> CompactTile {
        CompactTile::Multi(ps.iter().cloned().collect(), mps.iter().cloned().collect(), aas.iter().cloned().collect())
    }
    const E: CompactTile = CompactTile::Empty;

    #[test]
    fn compact_row_cases() {
        // SPEC 1.4.1: harmaa I
        test_compact_row("PISIN", &["HIISI"], &[E, g('I'), y('I'), y('S'), h('I')], &[]);
        // bug repro, fixed in 44be22139d5e6c4ff587cc2133293bacc97c0b65
        test_compact_row("LAHTI", &["KAALI", "PALVI"], &[E, g('A'), y('L'), b('L'), g('I')], &[]);
        // brown H suppressed (KAU**H**A) when green (**H**IENO) accounts for exact count of Hs (i.e. 1, known from absent H in HU**H**TA)
        test_compact_row("HURJA", &["HIENO", "KAUHA", "HUHTA"], &[g('H'), g('U'), b('U'), E, g('A')], &[]);
        // there should be no displaced A when there's already two green A's
        test_compact_row("SALPA", &["SUOMI", "HAHMO", "KOIPI", "OIKEA", "OHJAS"], &[g('S'), g('A'), E, g('P'), g('A')], &[]);
        // SPEC 1.3.1: L appears 2x as yellow but max 1 per guess → brown
        test_compact_row("LAHTI", &["KAALI", "PALVI"], &[E, g('A'), y('L'), b('L'), g('I')], &[]);
        // SPEC 1.3: I appears 2x as yellow but max 1 per guess → brown
        test_compact_row("LEIPÄ", &["PILLI", "LAPSI"], &[g('L'), y('I'), y('P'), E, h('I')], &[]);
        // SPEC 1.2: displaced yellows go to extra cell
        test_compact_row("MÄÄRÄ", &["ÄÄLIÖ", "RAMPA", "MÖKKI"], &[g('M'), g('Ä'), b('M'), E, E], &['R', 'Ä']);
        // SPEC 1.1.1: two yellows in same cell
        test_compact_row("LAHTI", &["KAALI", "TARHA"], &[y('T'), g('A'), E, m(&['L', 'H'], &[], &[]), g('I')], &[]);
        // one green, four yellow
        test_compact_row("HANHI", &["HIHNA"], &[g('H'), y('I'), y('H'), y('N'), y('A')], &[]);
        // duplicate yellow in same position deduped
        test_compact_row("LAHTI", &["KAALI", "MAALI"], &[E, g('A'), E, y('L'), g('I')], &[]);
        // all green
        test_compact_row("LAHTI", &["KAALI", "LAHTI"], &[g('L'), g('A'), g('H'), g('T'), g('I')], &[]);
    }

    #[test]
    fn test_serialization() {
        let words = ["LAHTI", "HANHI"];
        let mut m = make_test_monuli(&words);
        type_and_submit(&mut m, "KAKKU");

        let serialized = serde_json::to_string(&m).expect("Serialization failed");
        let mut deserialized: Monuli = serde_json::from_str(&serialized).expect("Deserialization failed");

        // Re-inject word lists as they are skipped
        deserialized.word_lists = m.word_lists.clone();
        deserialized.refresh();

        assert_eq!(deserialized.n_words, m.n_words);
        assert_eq!(deserialized.current_guess, m.current_guess);
        assert_eq!(deserialized.words[0].word, m.words[0].word);
        assert_eq!(deserialized.words[0].guesses, m.words[0].guesses);

        // known_states should be rebuilt by refresh()
        assert_eq!(deserialized.words[0].known_states, m.words[0].known_states);
    }
}
