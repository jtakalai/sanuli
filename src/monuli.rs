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

/// One cell in a compact row (SPEC 1.1). Per position: either green, or one or more yellows, or empty.
#[derive(Clone, Debug, PartialEq)]
pub enum CompactCell {
    Empty,
    Green(char),
    YellowOne(char),
    /// 2–4 yellow letters in this cell (from different guesses); UI renders as 2×2 grid.
    Yellows(Vec<char>),
}

/// Compact one-row summary for a word (SPEC 1.1). Greens at correct positions; yellows
/// per position (which wrong-position letter landed in which cell). Same cell can have
/// multiple yellows from different guesses.
pub fn compact_row(
    word_length: usize,
    guesses: &[Vec<(char, TileState)>],
) -> Vec<CompactCell> {
    let mut green_at: Vec<Option<char>> = vec![None; word_length];
    let mut yellow_at: Vec<Vec<char>> = (0..word_length).map(|_| Vec::new()).collect();
    for row in guesses.iter() {
        if row.len() != word_length {
            continue;
        }
        for (i, &(c, state)) in row.iter().enumerate() {
            match state {
                TileState::Correct => green_at[i] = Some(c),
                TileState::Present => yellow_at[i].push(c),
                _ => {}
            }
        }
    }
    (0..word_length)
        .map(|i| {
            if let Some(c) = green_at[i] {
                CompactCell::Green(c)
            } else {
                match yellow_at[i].len() {
                    0 => CompactCell::Empty,
                    1 => CompactCell::YellowOne(yellow_at[i][0]),
                    n => CompactCell::Yellows(yellow_at[i][..n].to_vec()),
                }
            }
        })
        .collect()
}

/// Per-word state: same structure as Sanuli for one word (guesses, known_states, known_counts).
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct MonuliWordState {
    pub word: Vec<char>,
    pub guesses: Vec<Vec<(char, TileState)>>,
    pub known_states: Vec<KnownStates>,
    pub known_counts: Vec<KnownCounts>,
}

impl MonuliWordState {
    fn is_solved(&self) -> bool {
        self.known_states
            .last()
            .map(|s| {
                (0..self.word.len()).all(|i| {
                    s.get(&(self.word[i], i)) == Some(&CharacterState::Correct)
                })
            })
            .unwrap_or(false)
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
    pub fn n_words(&self) -> usize {
        self.n_words
    }

    pub fn max_guesses(&self) -> usize {
        self.n_words + 1
    }

    pub fn current_guess_index(&self) -> usize {
        self.current_guess
    }

    pub fn word_state(&self, word_index: usize) -> Option<&MonuliWordState> {
        self.words.get(word_index)
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
        let mut solved: Vec<usize> = self
            .words
            .iter()
            .enumerate()
            .filter(|(_, w)| w.is_solved())
            .map(|(i, _)| i)
            .collect();
        unsolved.append(&mut solved);
        unsolved
    }

    /// Compact row for one word (for monuli list view). SPEC 1.1: per-position greens/yellows.
    pub fn compact_row(&self, word_index: usize) -> Vec<CompactCell> {
        match self.words.get(word_index) {
            Some(w) => {
                let submitted: Vec<_> = w
                    .guesses
                    .iter()
                    .take(self.current_guess)
                    .filter(|row| row.len() == self.word_length)
                    .cloned()
                    .collect();
                compact_row(self.word_length, &submitted)
            }
            None => vec![CompactCell::Empty; self.word_length],
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
    }
    fn board_for_word(&self, word_index: usize) -> Option<Board> {
        let w = self.words.get(word_index)?;
        let guesses = w.guesses[..=self.current_guess.min(w.guesses.len().saturating_sub(1))]
            .to_vec();
        Some(Board {
            guesses,
            current_guess: self.current_guess,
            is_guessing: self.is_guessing(),
        })
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

        // When in sanuli view, only the selected word has the current row; copy it to all words for evaluation.
        if let Some(src) = self.selected_word_index.filter(|&i| i < self.words.len()) {
            let row = self.words[src].guesses[self.current_guess].clone();
            for w in self.words.iter_mut() {
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
                .filter(|(_, w)| w.word == guess_letters)
                .map(|(i, _)| i)
                .collect();
            for i in to_replace {
                self.replace_word(i);
            }
        }

        for w in self.words.iter_mut() {
            if w.guesses[self.current_guess].len() == self.word_length {
                game::update_known_information(
                    &mut w.known_states,
                    &mut w.known_counts,
                    &mut w.guesses[self.current_guess],
                    self.current_guess,
                    &w.word,
                    max_guesses,
                );
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
    use crate::game::Game;
    use std::collections::HashSet;

    // if you add need more/new words in the tests, add them here first!
    const WORDS: &[&str] = &["LAHTI", "KAALI", "TARHA", "HANHI", "HIHNA"];

    fn test_word_lists() -> Rc<WordLists> {
        let set: HashSet<Vec<char>> = WORDS.iter().map(|s| s.chars().collect()).collect();
        let mut map = HashMap::new();
        map.insert((WordList::Full, 5), set);
        Rc::new(map)
    }

    /// SPEC 1.1 example 1.1.1: word LAHTI, guess 1 KAALI (L wrong at pos 3), guess 2 TARHA (H wrong at pos 3).
    /// After two guesses: yellow T, green A, empty, 2×2 [L, H], green I.
    #[test]
    fn compact_row_example_1_1_1() {
        let word_lists = test_word_lists();
        let mut monuli = Monuli::new_with_words(
            5,
            vec!["LAHTI".chars().collect()],
            WordList::Full,
            word_lists,
        );
        for c in "KAALI".chars() {
            monuli.push_character(c);
        }
        monuli.submit_guess();
        for c in "TARHA".chars() {
            monuli.push_character(c);
        }
        monuli.submit_guess();
        let out = monuli.compact_row(0);
        assert_eq!(out.len(), 5);
        assert_eq!(out[0], CompactCell::YellowOne('T'));
        assert_eq!(out[1], CompactCell::Green('A'));
        assert_eq!(out[2], CompactCell::Empty);
        assert_eq!(out[3], CompactCell::Yellows(vec!['L', 'H']));
        assert_eq!(out[4], CompactCell::Green('I'));
    }

    #[test]
    fn compact_row_all_green() {
        let word_lists = test_word_lists();
        let mut monuli = Monuli::new_with_words(
            5,
            vec!["LAHTI".chars().collect()],
            WordList::Full,
            word_lists,
        );
        // First guess wrong so the first-guess rule doesn't replace the word; second guess correct.
        for c in "KAALI".chars() {
            monuli.push_character(c);
        }
        monuli.submit_guess();
        for c in "LAHTI".chars() {
            monuli.push_character(c);
        }
        monuli.submit_guess();
        let out = monuli.compact_row(0);
        assert_eq!(out[0], CompactCell::Green('L'));
        assert_eq!(out[1], CompactCell::Green('A'));
        assert_eq!(out[2], CompactCell::Green('H'));
        assert_eq!(out[3], CompactCell::Green('T'));
        assert_eq!(out[4], CompactCell::Green('I'));
    }

    #[test]
    fn compact_row_one_green_four_yellow() {
        // Word HANHI. Guess HIHNA -> H correct, I present, H present, N present, A present.
        let word_lists = test_word_lists();
        let mut monuli = Monuli::new_with_words(
            5,
            vec!["HANHI".chars().collect()],
            WordList::Full,
            word_lists,
        );
        for c in "HIHNA".chars() {
            monuli.push_character(c);
        }
        monuli.submit_guess();
        let out = monuli.compact_row(0);
        assert_eq!(out[0], CompactCell::Green('H'));
        assert_eq!(out[1], CompactCell::YellowOne('I'));
        assert_eq!(out[2], CompactCell::YellowOne('H'));
        assert_eq!(out[3], CompactCell::YellowOne('N'));
        assert_eq!(out[4], CompactCell::YellowOne('A'));
    }

    #[test]
    fn compact_row_empty() {
        let word_lists = test_word_lists();
        let monuli = Monuli::new_with_words(
            5,
            vec!["LAHTI".chars().collect()],
            WordList::Full,
            word_lists,
        );
        let out = monuli.compact_row(0);
        assert_eq!(out.len(), 5);
        for i in 0..5 {
            assert_eq!(out[i], CompactCell::Empty);
        }
    }
}
