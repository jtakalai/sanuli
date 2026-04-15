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

/// One cell in a compact row (SPEC 1.1). Per position: either green, or one or more yellows/browns, or empty.
#[derive(Clone, Debug, PartialEq)]
pub enum CompactCell {
    Empty,
    Green(char),
    YellowOne(char),
    /// SPEC 1.3: letter appears more times as yellow than any single guess supports.
    BrownOne(char),
    /// 2–4 yellow/brown letters in this cell; bool = is_brown (SPEC 1.3).
    Yellows(Vec<(char, bool)>),
}

/// Compact one-row summary for a word (SPEC 1.1 + 1.2).
/// Built from guesses only (no knowledge of the correct word).
/// Greens at correct positions; yellows shown where they appeared.
/// A yellow is suppressed only when greens fully account for all
/// observed instances of that letter (observed_count == green_count).
/// SPEC 1.2: yellows displaced by greens go to an extra cell if
/// the letter has no yellow at any non-green position.
pub fn compact_row(
    guesses: &[Vec<(char, TileState)>],
) -> (Vec<CompactCell>, Option<CompactCell>) {
    let word_length = guesses[0].len();
    assert!(guesses.iter().all(|r| r.len() == word_length), "all guesses must be the same length");
    let mut green_at: Vec<Option<char>> = vec![None; word_length];
    let mut yellow_at: Vec<Vec<char>> = (0..word_length).map(|_| Vec::new()).collect();
    let mut observed_count_of: HashMap<char, usize> = HashMap::new();

    for row in guesses.iter() {
        let mut row_count_of: HashMap<char, usize> = HashMap::new();
        for (i, &(c, state)) in row.iter().enumerate() {
            match state {
                TileState::Correct => {
                    green_at[i] = Some(c);
                    *row_count_of.entry(c).or_insert(0) += 1;
                }
                TileState::Present => {
                    if !yellow_at[i].contains(&c) {
                        yellow_at[i].push(c);
                    }
                    *row_count_of.entry(c).or_insert(0) += 1;
                }
                _ => {}
            }
        }
        for (c, count) in row_count_of {
            let entry = observed_count_of.entry(c).or_insert(0);
            *entry = (*entry).max(count);
        }
    }

    let green_count_of = |c: char| green_at.iter().filter(|g| **g == Some(c)).count();

    // SPEC 1.2: save yellows displaced by greens.
    let mut displaced: Vec<char> = Vec::new();
    for i in 0..word_length {
        if green_at[i].is_some() {
            displaced.extend(yellow_at[i].drain(..));
        }
    }

    // Suppress yellows only when greens fully account for observations.
    for i in 0..word_length {
        if green_at[i].is_some() {
            continue;
        }
        yellow_at[i].retain(|&c| {
            observed_count_of.get(&c).copied().unwrap_or(0) != green_count_of(c)
        });
    }

    // SPEC 1.2: displaced yellows that aren't yellow elsewhere go to extra cell.
    let mut extra: Vec<char> = Vec::new();
    let mut extra_seen: Vec<char> = Vec::new();
    for c in displaced {
        if observed_count_of.get(&c).copied().unwrap_or(0) == green_count_of(c) {
            continue;
        }
        let has_yellow_elsewhere = yellow_at.iter().any(|ys| ys.contains(&c));
        if !has_yellow_elsewhere && !extra_seen.contains(&c) {
            extra_seen.push(c);
            extra.push(c);
        }
    }

    // SPEC 1.3: determine which letters should be brown.
    // Count total yellow occurrences per letter across compact row + extra.
    let mut compact_yellow_count: HashMap<char, usize> = HashMap::new();
    for pos_yellows in &yellow_at {
        for &c in pos_yellows {
            *compact_yellow_count.entry(c).or_insert(0) += 1;
        }
    }
    for &c in &extra {
        *compact_yellow_count.entry(c).or_insert(0) += 1;
    }
    // For each letter, find max yellow (Present) count in any single guess.
    let mut max_guess_yellow: HashMap<char, usize> = HashMap::new();
    for row in guesses {
        let mut guess_count: HashMap<char, usize> = HashMap::new();
        for &(c, state) in row {
            if state == TileState::Present {
                *guess_count.entry(c).or_insert(0) += 1;
            }
        }
        for (c, count) in guess_count {
            let entry = max_guess_yellow.entry(c).or_insert(0);
            *entry = (*entry).max(count);
        }
    }
    let is_brown = |c: char| -> bool {
        compact_yellow_count.get(&c).copied().unwrap_or(0)
            > max_guess_yellow.get(&c).copied().unwrap_or(0)
    };

    let row: Vec<CompactCell> = (0..word_length)
        .map(|i| {
            if let Some(c) = green_at[i] {
                CompactCell::Green(c)
            } else {
                match yellow_at[i].len() {
                    0 => CompactCell::Empty,
                    1 => {
                        let c = yellow_at[i][0];
                        if is_brown(c) { CompactCell::BrownOne(c) } else { CompactCell::YellowOne(c) }
                    }
                    n => CompactCell::Yellows(
                        yellow_at[i][..n].iter().map(|&c| (c, is_brown(c))).collect()
                    ),
                }
            }
        })
        .collect();

    let extra_cell = match extra.len() {
        0 => None,
        1 => {
            let c = extra[0];
            Some(if is_brown(c) { CompactCell::BrownOne(c) } else { CompactCell::YellowOne(c) })
        }
        _ => Some(CompactCell::Yellows(extra.iter().map(|&c| (c, is_brown(c))).collect())),
    };

    (row, extra_cell)
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

    /// Order for list view: unsolved first (by index), then solved in solve order.
    pub fn word_order(&self) -> Vec<usize> {
        let mut unsolved: Vec<usize> = self
            .words
            .iter()
            .enumerate()
            .filter(|(_, w)| !w.is_solved())
            .map(|(i, _)| i)
            .collect();
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
    pub fn compact_row(&self, word_index: usize) -> (Vec<CompactCell>, Option<CompactCell>) {
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
                    return (vec![CompactCell::Empty; self.word_length], None);
                }
                compact_row(&submitted)
            }
            None => (vec![CompactCell::Empty; self.word_length], None),
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
        let max_guesses = n_words + 1;
        let mut words = Vec::with_capacity(n_words);
        let mut used = HashSet::new();
        for _ in 0..n_words {
            let word = get_random_word_excluding(
                word_list,
                word_length,
                allow_profanities,
                &word_lists,
                &used,
            )
            .unwrap_or_else(|| vec!['X'; word_length]);
            used.insert(word.clone());
            let guesses = std::iter::repeat(Vec::with_capacity(word_length))
                .take(max_guesses)
                .collect::<Vec<_>>();
            let known_states = std::iter::repeat(HashMap::new())
                .take(max_guesses)
                .collect::<Vec<_>>();
            let known_counts = std::iter::repeat(HashMap::new())
                .take(max_guesses)
                .collect::<Vec<_>>();
            words.push(MonuliWordState {
                word,
                guesses,
                known_states,
                known_counts,
                solved_at: None,
            });
        }
        Self {
            game_mode: GameMode::Monuli(n_words),
            word_list,
            word_length,
            n_words,
            words,
            current_guess: 0,
            streak: 0,
            message: String::new(),
            allow_profanities,
            word_lists,
            selected_word_index: None,
            show_overview: n_words > 10,
        }
    }

    /// Create a Monuli with fixed words (for tests). Caller must provide word_lists
    /// that include any guess words so submit_guess accepts them.
    #[cfg(test)]
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
            show_overview: n_words > 10,
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
        game.show_overview = game.n_words > 10;
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
            Some(w) => KeyState::Single(game::keyboard_tile_state(
                key,
                self.current_guess,
                &w.known_states,
                &w.known_counts,
            )),
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

    fn test_compact_row(word: &str, guesses: &[&str], expected: &[CompactCell], expected_extra: Option<CompactCell>) {
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

    fn g(c: char) -> CompactCell { CompactCell::Green(c) }
    fn y(c: char) -> CompactCell { CompactCell::YellowOne(c) }
    fn b(c: char) -> CompactCell { CompactCell::BrownOne(c) }
    fn ys(cs: &[char]) -> CompactCell { CompactCell::Yellows(cs.iter().map(|&c| (c, false)).collect()) }
    const E: CompactCell = CompactCell::Empty;

    #[test]
    fn compact_row_cases() {
        // all green
        test_compact_row("LAHTI", &["KAALI", "LAHTI"], &[g('L'), g('A'), g('H'), g('T'), g('I')], None);
        // SPEC 1.1.1: two yellows in same cell
        test_compact_row("LAHTI", &["KAALI", "TARHA"], &[y('T'), g('A'), E, ys(&['L', 'H']), g('I')], None);
        // one green, four yellow
        test_compact_row("HANHI", &["HIHNA"], &[g('H'), y('I'), y('H'), y('N'), y('A')], None);
        // duplicate yellow in same position deduped
        test_compact_row("LAHTI", &["KAALI", "MAALI"], &[E, g('A'), E, y('L'), g('I')], None);
        // yellow suppressed when letter already green
        test_compact_row("HURJA", &["HIENO", "KAUHA", "HUHTA"], &[g('H'), g('U'), E, E, g('A')], None);
        // SPEC 1.3: I appears 2x as yellow but max 1 per guess → brown
        test_compact_row("LEIPÄ", &["PILLI", "LAPSI"], &[g('L'), b('I'), y('P'), E, b('I')], None);
        // SPEC 1.2: displaced yellows go to extra cell
        test_compact_row("MÄÄRÄ", &["ÄÄLIÖ", "RAMPA", "MÖKKI"],
            &[g('M'), g('Ä'), E, E, E], Some(ys(&['Ä', 'R'])));
        // SPEC 1.3.1: L appears 2x as yellow but max 1 per guess → brown
        test_compact_row("LAHTI", &["KAALI", "PALVI"], &[E, g('A'), b('L'), b('L'), g('I')], None);
    }
}
