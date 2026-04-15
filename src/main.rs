use std::collections::HashMap;
use std::str::FromStr;
use wasm_bindgen::{prelude::Closure, JsCast};
use web_sys::{window, Window};
use yew::prelude::*;

use game::Game;

mod components;
mod game;
mod manager;
mod monuli;
mod neluli;
mod sanuli;

use components::{
    board::Board,
    header::Header,
    keyboard::{Keyboard, EnterButtonState},
    modal::{HelpModal, MenuModal},
    monuli::MonuliView,
};
use manager::{ControlKey, GameMode, KeyState, Manager, Theme, WordList};
use monuli::Monuli;

const ALLOWED_GUESS_KEYS: [char; 28] = [
    'Q', 'W', 'E', 'R', 'T', 'Y', 'U', 'I', 'O', 'P', 'A', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L',
    'Ö', 'Ä', 'Z', 'X', 'C', 'V', 'B', 'N', 'M',
];

pub enum Msg {
    KeyPress(char),
    Backspace,
    Enter,
    Guess,
    NextWord,
    ToggleHelp,
    ToggleMenu,
    ChangeGameMode(GameMode),
    ChangePreviousGameMode,
    ChangeWordLength(usize),
    ChangeWordList(WordList),
    ChangeAllowProfanities(bool),
    ChangeTheme(Theme),
    ShareEmojis,
    ShareLink,
    RevealHiddenTiles,
    ResetGame,
    /// Move from monuli list view to sanuli view (if given word index), or back to list view (if None).
    SelectMonuliWord(Option<usize>),
    /// ControlKeyPress events can be interpreted by the GameMode to implement a keyboard-driven UI
    ControlKeyPress(ControlKey),
    /// Auto-sorting helps selecting good words to crack in large monulis
    ToggleAutoSort,
}

pub struct App {
    manager: Manager,
    is_help_visible: bool,
    is_menu_visible: bool,
    is_emojis_copied: bool,
    is_link_copied: bool,
    keyboard_listener: Option<Closure<dyn Fn(KeyboardEvent)>>,
}

impl Component for App {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            manager: Manager::new(),
            is_help_visible: false,
            is_menu_visible: false,
            is_emojis_copied: false,
            is_link_copied: false,
            keyboard_listener: None,
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if let Some(g) = self.manager.game.as_mut() {
            if let Some(monuli) = g.as_any_mut().downcast_mut::<Monuli>() {
                if let Some(target) = monuli.list_scroll_to.take() {
                    monuli_scroll(".monuli-word-list", Some(target), None, true, 0.0);
                } else if let Some(cursor) = monuli.list_ensure_visible.take() {
                    monuli_scroll(".monuli-word-list", Some(cursor), None, false, 2.0);
                }

                if monuli.selected_word_index.is_some() {
                    monuli_scroll(".monuli-sanuli-view", None, Some(".monuli-sanuli-view .current"), false, 0.5);
                }
            }
        }

        if !first_render {
            return;
        }

        let window: Window = window().expect("window not available");

        let cb = ctx.link().batch_callback(|e: KeyboardEvent| {
            if e.key().chars().count() == 1 {
                let key = e.key().to_uppercase().chars().next().unwrap();
                if ALLOWED_GUESS_KEYS.contains(&key) && !e.ctrl_key() && !e.alt_key() && !e.meta_key() {
                    e.prevent_default();
                    Some(Msg::KeyPress(key))
                } else {
                    None
                }
            } else if e.key() == "Backspace" {
                e.prevent_default();
                Some(Msg::Backspace)
            } else if e.key() == "Enter" {
                e.prevent_default();
                Some(Msg::Enter)
            } else if let Ok(key) = ControlKey::from_str(e.key().as_str()) {
                e.prevent_default();
                Some(Msg::ControlKeyPress(key))
            } else {
                None
            }
        });

        let listener =
            Closure::<dyn Fn(KeyboardEvent)>::wrap(Box::new(move |e: KeyboardEvent| cb.emit(e)));

        window
            .add_event_listener_with_callback("keydown", listener.as_ref().unchecked_ref())
            .unwrap();
        self.keyboard_listener = Some(listener);
    }

    fn destroy(&mut self, _: &Context<Self>) {
        // Remove the keyboard listener
        if let Some(listener) = self.keyboard_listener.take() {
            let window: Window = window().expect("window not available");
            window
                .remove_event_listener_with_callback("keydown", listener.as_ref().unchecked_ref())
                .unwrap();
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::KeyPress(c) => self.manager.push_character(c),
            Msg::Backspace => self.manager.pop_character(),
            Msg::Enter => {
                let link = ctx.link();

                if let Some(game) = &self.manager.game {
                    if game.is_guessing() {
                        link.send_message(Msg::Guess);
                    } else {
                        if matches!(game.game_mode(), GameMode::DailyWord(_) | GameMode::Shared) {
                            link.send_message(Msg::ChangePreviousGameMode);
                        } else if matches!(game.game_mode(), GameMode::Monuli(_)) {
                            link.send_message(Msg::ControlKeyPress(ControlKey::Enter));
                        } else {
                            link.send_message(Msg::NextWord);
                        }
                    }
                }
            }
            Msg::Guess => self.manager.submit_guess(),
            Msg::NextWord => {
                self.manager.next_word();
                self.is_emojis_copied = false;
                self.is_link_copied = false;
            }
            Msg::ToggleHelp => {
                self.is_help_visible = !self.is_help_visible;
                self.is_menu_visible = false;
            }
            Msg::ToggleMenu => {
                self.is_menu_visible = !self.is_menu_visible;
                self.is_help_visible = false;
            }
            Msg::ChangeWordLength(new_length) => {
                self.manager.change_word_length(new_length);
                self.is_menu_visible = false;
                self.is_help_visible = false;
            }
            Msg::ChangeGameMode(new_mode) => {
                self.manager.change_game_mode(new_mode);
                self.is_menu_visible = false;
                self.is_help_visible = false;
            }
            Msg::ChangeWordList(new_list) => {
                self.manager.change_word_list(new_list);
                self.is_menu_visible = false;
                self.is_help_visible = false;
            }
            Msg::ChangePreviousGameMode => {
                self.manager.change_previous_game_mode();
                self.is_emojis_copied = false;
                self.is_link_copied = false;
            }
            Msg::ChangeAllowProfanities(is_allowed) => {
                self.manager.change_allow_profanities(is_allowed);
                self.is_menu_visible = false;
                self.is_help_visible = false;
            }
            Msg::ChangeTheme(theme) => self.manager.change_theme(theme),
            Msg::ShareEmojis => {
                #[cfg(web_sys_unstable_apis)]
                {
                    use web_sys::Navigator;

                    if let Some(emojis) = self.manager.share_emojis() {
                        let window: Window = window().expect("window not available");
                        let navigator: Navigator = window.navigator();
                        let _promise = navigator.clipboard().write_text(emojis.as_str());
                    }
                }
                self.is_emojis_copied = true;
                self.is_link_copied = false;
            }
            Msg::ShareLink => {
                #[cfg(web_sys_unstable_apis)]
                {
                    use web_sys::Navigator;

                    if let Some(link) = self.manager.share_link() {
                        let window: Window = window().expect("window not available");
                        let navigator: Navigator = window.navigator();
                        let _promise = navigator.clipboard().write_text(link.as_str());
                    }
                }
                self.is_link_copied = true;
                self.is_emojis_copied = false;
            }
            Msg::RevealHiddenTiles => self.manager.reveal_hidden_tiles(),
            Msg::ResetGame => self.manager.reset_game(),
            Msg::SelectMonuliWord(idx) => {
                if let Some(g) = self.manager.game.as_mut() {
                    if let Some(monuli) = g.as_any_mut().downcast_mut::<Monuli>() {
                        if let Some(word_idx) = idx {
                            monuli.select_monuli_word(word_idx);
                        } else {
                            monuli.deselect_monuli_word();
                        }
                    }
                }
            }
            Msg::ControlKeyPress(control_key) => {
                if let Some(game) = self.manager.game.as_mut() {
                    let msgs = game.control_key_press(control_key);
                    if !msgs.is_empty() {
                        ctx.link().send_message_batch(msgs);
                    }
                }
            }
            Msg::ToggleAutoSort => {
                if let Some(g) = self.manager.game.as_mut() {
                    if let Some(monuli) = g.as_any_mut().downcast_mut::<Monuli>() {
                        monuli.auto_sort = !monuli.auto_sort;
                        let _ = monuli.persist();
                    }
                }
            }
        };

        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        if let Some(game) = &self.manager.game {
            let keyboard_state: HashMap<char, KeyState> = ALLOWED_GUESS_KEYS
                .iter()
                .map(|key| (*key, game.keyboard_tilestate(key)))
                .collect();

            let last_guess = game.last_guess();

            let boards = game.boards();

            let monuli_word_solved = if let Some(monuli) = game.as_any().downcast_ref::<Monuli>() {
                monuli.selected_word_index
                    .map(|idx| monuli.monuli_word_is_solved(idx))
                    .unwrap_or(false)
            } else {
                false
            };
            let enter_button_state = if monuli_word_solved {
                EnterButtonState::Return
            } else if game.is_guessing() {
                EnterButtonState::Guess
            } else if matches!(game.game_mode(), GameMode::DailyWord(_) | GameMode::Shared) {
                EnterButtonState::Return
            } else {
                EnterButtonState::New
            };

            html! {
                <div class={classes!("game", self.manager.theme.to_string())}>
                    <Header
                        on_toggle_help_cb={link.callback(|_| Msg::ToggleHelp)}
                        on_toggle_menu_cb={link.callback(|_| Msg::ToggleMenu)}
                        title={game.title()}
                    />

                    {
                        match (game.game_mode(), boards.len()) {
                            (GameMode::Monuli(_), _) => {
                                if let Some(monuli) = game.as_any().downcast_ref::<Monuli>() {
                                    html! {
                                        <MonuliView
                                            game={monuli.clone()}
                                            last_guess={last_guess.clone()}
                                            callback={link.callback(move |msg| msg)}
                                            max_guesses={game.max_guesses()}
                                        />
                                    }
                                } else {
                                    html! { <div class="board-container"><p class="monuli-placeholder">{"Monuli"}</p></div> }
                                }
                            },
                            (_, 1) => html! {
                                <div class="board-container">
                                    <Board
                                        guesses={boards[0].guesses.clone()}
                                        is_guessing={boards[0].is_guessing}
                                        current_guess={boards[0].current_guess}
                                        is_reset={game.is_reset()}
                                        is_hidden={game.is_hidden()}
                                        previous_guesses={game.previous_guesses().clone()}
                                        max_guesses={game.max_guesses()}
                                        word_length={game.word_length()}
                                    />
                                </div>
                            },
                            (_, 4) => html! {
                                <div class="quadruple-container">
                                    <div class="quadruple-grid">
                                        {game.boards().iter().map(|board| {
                                            html! {
                                                <Board
                                                    guesses={board.guesses.clone()}
                                                    is_guessing={board.is_guessing}
                                                    current_guess={board.current_guess}
                                                    is_reset={game.is_reset()}
                                                    is_hidden={game.is_hidden()}
                                                    previous_guesses={game.previous_guesses().clone()}
                                                    max_guesses={game.max_guesses()}
                                                    word_length={game.word_length()}
                                                />
                                            }
                                        }).collect::<Html>()}
                                    </div>
                                </div>
                            },
                            _ => html! {}
                        }
                    }

                    <Keyboard
                        callback={link.callback(move |msg| msg)}
                        is_unknown={game.is_unknown()}
                        is_winner={game.is_winner()}
                        is_guessing={game.is_guessing()}
                        is_hidden={game.is_hidden()}
                        is_emojis_copied={self.is_emojis_copied}
                        is_link_copied={self.is_link_copied}
                        game_mode={game.game_mode().clone()}
                        enter_button_state={enter_button_state}
                        message={game.message()}
                        word={game.word().iter().collect::<String>()}
                        last_guess={last_guess}
                        keyboard={keyboard_state}
                    />

                    {
                        if self.is_help_visible {
                            html! { <HelpModal theme={self.manager.theme} callback={link.callback(move |msg| msg)} game_mode={self.manager.current_game_mode} /> }
                        } else {
                            html! {}
                        }
                    }

                    {
                        if self.is_menu_visible {
                            let monuli_auto_sort = game.as_any().downcast_ref::<Monuli>()
                                .map(|m| m.auto_sort).unwrap_or(false);
                            html! {
                                <MenuModal
                                    callback={link.callback(move |msg| msg)}
                                    game_mode={self.manager.current_game_mode}
                                    word_length={self.manager.current_word_length}
                                    current_word_list={self.manager.current_word_list}
                                    allow_profanities={self.manager.allow_profanities}
                                    theme={self.manager.theme}
                                    max_streak={self.manager.max_streak}
                                    total_played={self.manager.total_played}
                                    total_solved={self.manager.total_solved}
                                    monuli_auto_sort={monuli_auto_sort}
                                    monuli_n={self.manager.last_monuli_n}
                                />
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>
            }
        } else {
            html! {
                <MenuModal
                    callback={link.callback(move |msg| msg)}
                    game_mode={self.manager.current_game_mode}
                    word_length={self.manager.current_word_length}
                    current_word_list={self.manager.current_word_list}
                    allow_profanities={self.manager.allow_profanities}
                    theme={self.manager.theme}
                    max_streak={self.manager.max_streak}
                    total_played={self.manager.total_played}
                    total_solved={self.manager.total_solved}
                    monuli_n={self.manager.last_monuli_n}
                />
            }
        }
    }
}

/// Scrolling helper for Monuli mode: often things take more space than what fits on screen
/// This scrolls the monuli view to target index e.g. when using keyboard to navigate, or when returning to list view
fn monuli_scroll(container_selector: &str, target_index: Option<usize>, target_selector: Option<&str>, center: bool, margin_rows: f64) {
    let window = match window() { Some(w) => w, None => return };
    let document = match window.document() { Some(d) => d, None => return };
    let container = match document.query_selector(container_selector).ok().flatten() {
        Some(el) => el.dyn_into::<web_sys::HtmlElement>().unwrap(),
        None => return
    };

    let target_el = if let Some(idx) = target_index {
        container.children().item(idx as u32).and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
    } else if let Some(sel) = target_selector {
        document.query_selector(sel).ok().flatten().and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
    } else {
        None
    };

    if let Some(el) = target_el {
        let el_html = el.dyn_ref::<web_sys::HtmlElement>().unwrap();
        let container_html = container.dyn_ref::<web_sys::HtmlElement>().unwrap();
        let row_top = el_html.offset_top() as f64 - container_html.offset_top() as f64;
        let row_height = el_html.offset_height() as f64;
        let container_height = container_html.client_height() as f64;
        let scroll_top = container_html.scroll_top() as f64;

        if center {
            let scroll_to = (row_top - container_height / 2.0 + row_height / 2.0).max(0.0);
            container.set_scroll_top(scroll_to as i32);
        } else {
            let row_bottom = row_top + row_height;
            let margin = margin_rows * row_height;
            if row_top < scroll_top + margin {
                container.set_scroll_top((row_top - margin).max(0.0) as i32);
            } else if row_bottom > scroll_top + container_height - margin {
                container.set_scroll_top((row_bottom + margin - container_height) as i32);
            }
        }
    }
}

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<App>::new().render();
}
