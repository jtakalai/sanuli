use std::collections::HashMap;
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
    keyboard::Keyboard,
    modal::{HelpModal, MenuModal},
};
use manager::{GameMode, KeyState, Manager, Theme, WordList};
use monuli::{CompactTile, Monuli};

const ALLOWED_KEYS: [char; 28] = [
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
    SelectMonuliWord(Option<usize>),
    /// (show, optional word_order index to focus when closing)
    MonuliSetOverview(bool, Option<usize>),
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
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
                    scroll_monuli_word_list_to(target);
                } else if let Some(cursor) = monuli.list_ensure_visible.take() {
                    ensure_cursor_visible_in_list(cursor);
                }

                if monuli.selected_word_index.is_some() {
                    ensure_sanuli_current_visible();
                }
            }
        }

        setup_overview_hover();

        if !first_render {
            return;
        }

        let window: Window = window().expect("window not available");

        let cb = ctx.link().batch_callback(|e: KeyboardEvent| {
            if e.key().chars().count() == 1 {
                let key = e.key().to_uppercase().chars().next().unwrap();
                if ALLOWED_KEYS.contains(&key) && !e.ctrl_key() && !e.alt_key() && !e.meta_key() {
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
            } else if e.key() == "ArrowLeft" {
                e.prevent_default();
                Some(Msg::ArrowLeft)
            } else if e.key() == "ArrowRight" {
                e.prevent_default();
                Some(Msg::ArrowRight)
            } else if e.key() == "ArrowUp" {
                e.prevent_default();
                Some(Msg::ArrowUp)
            } else if e.key() == "ArrowDown" {
                e.prevent_default();
                Some(Msg::ArrowDown)
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
                    // In Monuli sanuli view for a solved word, Enter = go back
                    if matches!(game.game_mode(), GameMode::Monuli(_)) {
                        if let Some(idx) = game.monuli_selected_word() {
                            if game.monuli_word_is_solved(idx) {
                                link.send_message(Msg::SelectMonuliWord(None));
                                return true;
                            }
                        }
                    }

                    if game.is_guessing() {
                        link.send_message(Msg::Guess);
                    } else {
                        if matches!(game.game_mode(), GameMode::DailyWord(_) | GameMode::Shared) {
                            link.send_message(Msg::ChangePreviousGameMode);
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
                    if let (Some(word_idx), Some(monuli)) = (idx, g.as_any_mut().downcast_mut::<Monuli>()) {
                        if monuli.word_is_solved(word_idx) {
                            monuli.list_cursor = None;
                        } else {
                            let order = monuli.word_order();
                            if let Some(pos) = order.iter().position(|&i| i == word_idx) {
                                monuli.list_cursor = Some(pos);
                            }
                        }
                    }
                    g.set_monuli_selected_word(idx);
                }
            }
            Msg::MonuliSetOverview(show, focus_idx) => {
                if let Some(g) = self.manager.game.as_mut() {
                    if let Some(monuli) = g.as_any_mut().downcast_mut::<Monuli>() {
                        monuli.show_overview = show;
                        monuli.selected_word_index = None;
                        if !show {
                            if let Some(wo_idx) = focus_idx {
                                monuli.list_cursor = Some(wo_idx);
                                monuli.list_scroll_to = Some(wo_idx);
                            }
                        }
                    }
                }
            }
            Msg::ArrowLeft => {
                if let Some(g) = &self.manager.game {
                    if matches!(g.game_mode(), GameMode::Monuli(_)) && g.monuli_selected_word().is_some() {
                        ctx.link().send_message(Msg::SelectMonuliWord(None));
                    }
                }
            }
            Msg::ArrowRight => {
                if let Some(g) = self.manager.game.as_mut() {
                    if let Some(monuli) = g.as_any_mut().downcast_mut::<Monuli>() {
                        if monuli.selected_word_index.is_none() {
                            if let Some(cursor) = monuli.list_cursor {
                                let order = monuli.word_order();
                                if let Some(&word_idx) = order.get(cursor) {
                                    monuli.selected_word_index = Some(word_idx);
                                    monuli.show_overview = false;
                                }
                            }
                        }
                    }
                }
            }
            Msg::ArrowUp => {
                if let Some(g) = self.manager.game.as_mut() {
                    if let Some(monuli) = g.as_any_mut().downcast_mut::<Monuli>() {
                        if monuli.selected_word_index.is_none() {
                            let order = monuli.word_order();
                            let game_over = !monuli.is_guessing();
                            let n = if game_over { order.len() } else {
                                order.iter().filter(|&&i| !monuli.word_is_solved(i)).count()
                            };
                            if n > 0 {
                                let new_cursor = match monuli.list_cursor {
                                    None => n - 1,
                                    Some(0) => n - 1,
                                    Some(c) => (c - 1).min(n - 1),
                                };
                                monuli.list_cursor = Some(new_cursor);
                                monuli.list_ensure_visible = Some(new_cursor);
                            }
                            if monuli.show_overview {
                                monuli.show_overview = false;
                                monuli.list_scroll_to = monuli.list_cursor;
                            }
                        }
                    }
                }
            }
            Msg::ArrowDown => {
                if let Some(g) = self.manager.game.as_mut() {
                    if let Some(monuli) = g.as_any_mut().downcast_mut::<Monuli>() {
                        if monuli.selected_word_index.is_none() {
                            let order = monuli.word_order();
                            let game_over = !monuli.is_guessing();
                            let n = if game_over { order.len() } else {
                                order.iter().filter(|&&i| !monuli.word_is_solved(i)).count()
                            };
                            if n > 0 {
                                let new_cursor = match monuli.list_cursor {
                                    None => 0,
                                    Some(c) if c >= n - 1 => 0,
                                    Some(c) => c + 1,
                                };
                                monuli.list_cursor = Some(new_cursor);
                                monuli.list_ensure_visible = Some(new_cursor);
                            }
                            if monuli.show_overview {
                                monuli.show_overview = false;
                                monuli.list_scroll_to = monuli.list_cursor;
                            }
                        }
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
            let keyboard_state: HashMap<char, KeyState> = match (game.game_mode(), game.monuli_selected_word()) {
                (GameMode::Monuli(_), Some(idx)) => ALLOWED_KEYS
                    .iter()
                    .map(|key| (*key, game.keyboard_tilestate_for_word(idx, key)))
                    .collect(),
                _ => ALLOWED_KEYS
                    .iter()
                    .map(|key| (*key, game.keyboard_tilestate(key)))
                    .collect(),
            };

            let last_guess = game.last_guess();

            let boards = game.boards();

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
                                    if let Some(word_idx) = monuli.selected_word_index {
                                        // Sanuli view for a single word
                                        if let Some(board) = game.board_for_word(word_idx) {
                                            let onback = link.callback(move |e: MouseEvent| {
                                                e.prevent_default();
                                                Msg::SelectMonuliWord(None)
                                            });
                                            html! {
                                                <>
                                                    <div class="monuli-back-bar">
                                                        <button class="monuli-back-button" onmousedown={onback}>
                                                            {"← TAKAISIN"}
                                                        </button>
                                                    </div>
                                                    <div class="board-container monuli-sanuli-view">
                                                        <Board
                                                            guesses={board.guesses}
                                                            is_guessing={board.is_guessing}
                                                            current_guess={board.current_guess}
                                                            is_reset={false}
                                                            is_hidden={false}
                                                            previous_guesses={vec![]}
                                                            max_guesses={game.max_guesses()}
                                                            word_length={game.word_length()}
                                                            board_class={"board-monuli".to_string()}
                                                        />
                                                    </div>
                                                </>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    } else if monuli.show_overview {
                                        // Overview: compact grid of all words
                                        let word_length = game.word_length();
                                        let word_order = monuli.word_order();
                                        let n_words = word_order.len();
                                        let current_letters: Vec<char> = last_guess.chars().collect();

                                        let show_letters = n_words <= 20;

                                        let render_overview_cell = move |cell: &CompactTile| -> Html {
                                            let (class, letter) = match cell {
                                                CompactTile::Empty => ("overview-cell overview-cell-empty", None),
                                                CompactTile::Absent(c) => ("overview-cell overview-cell-empty", if show_letters { Some(*c) } else { None }),
                                                CompactTile::Correct(c) => ("overview-cell overview-cell-green", if show_letters { Some(*c) } else { None }),
                                                CompactTile::Yellow(c) => ("overview-cell overview-cell-yellow", if show_letters { Some(*c) } else { None }),
                                                CompactTile::Brown(c) => ("overview-cell overview-cell-brown", if show_letters { Some(*c) } else { None }),
                                                CompactTile::Multi(_ys, _bs, _as) => ("overview-cell overview-cell-yellow", None),
                                            };
                                            if let Some(ch) = letter {
                                                html! { <div class={class}>{ ch }</div> }
                                            } else {
                                                html! { <div class={class}></div> }
                                            }
                                        };

                                        let cell_size = if n_words <= 20 { 16 } else if n_words <= 50 { 10 } else { 7 };
                                        let row_width = word_length * cell_size + (word_length - 1) * 2;
                                        let style_var = format!("--overview-cell-size: {}px; --overview-row-width: {}px;", cell_size, row_width);

                                        // Transpose word_order into column-major order for 4-column grid
                                        let cols = 4usize;
                                        let k = (n_words + cols - 1) / cols;
                                        let mut transposed: Vec<(Option<usize>, usize)> = Vec::with_capacity(k * cols);
                                        for row in 0..k {
                                            for col in 0..cols {
                                                let src = col * k + row;
                                                let wo_idx = word_order.get(src).copied();
                                                transposed.push((wo_idx, src));
                                            }
                                        }

                                        html! {
                                            <div class="monuli-list-view">
                                                <div class={format!("row-{}", word_length)}>
                                                    { (0..word_length).map(|i| {
                                                        let c = current_letters.get(i).copied().unwrap_or(' ');
                                                        html! { <div class={classes!("tile", "current", "unknown")}>{ c }</div> }
                                                    }).collect::<Html>() }
                                                </div>
                                                <div class="monuli-overview-grid" style={style_var}
                                                     data-k={k.to_string()} data-cols={cols.to_string()} data-n={n_words.to_string()}>
                                                    { transposed.iter().map(|&(opt_word_index, wo_idx)| {
                                                        if let Some(_word_index) = opt_word_index {
                                                            let (compact, _extra) = monuli.compact_row(_word_index);
                                                            let onclick = link.callback(move |e: MouseEvent| {
                                                                e.prevent_default();
                                                                Msg::MonuliSetOverview(false, Some(wo_idx))
                                                            });
                                                            html! {
                                                                <div class="overview-row"
                                                                     data-wo={wo_idx.to_string()}
                                                                     onmousedown={onclick}>
                                                                    { compact.iter().map(&render_overview_cell).collect::<Html>() }
                                                                </div>
                                                            }
                                                        } else {
                                                            html! { <div class="overview-row overview-row-empty"></div> }
                                                        }
                                                    }).collect::<Html>() }
                                                </div>
                                            </div>
                                        }
                                    } else {
                                        // List view: single column, scrollable
                                        let word_length = game.word_length();
                                        let word_order = monuli.word_order();
                                        let n_words = word_order.len();
                                        let current_letters: Vec<char> = last_guess.chars().collect();
                                        let mut first_solved = word_order.len();
                                        for (pos, &idx) in word_order.iter().enumerate() {
                                            if monuli.word_is_solved(idx) {
                                                first_solved = pos;
                                                break;
                                            }
                                        }

                                        let list_cursor = monuli.list_cursor;
                                        let has_overview = n_words > 10;
                                        let on_show_all = link.callback(move |e: MouseEvent| {
                                            e.prevent_default();
                                            Msg::MonuliSetOverview(true, None)
                                        });

                                        html! {
                                            <div class="monuli-list-view">
                                                <div class={format!("row-{}", word_length)}>
                                                    { (0..word_length).map(|i| {
                                                        let c = current_letters.get(i).copied().unwrap_or(' ');
                                                        html! { <div class={classes!("tile", "current", "unknown")}>{ c }</div> }
                                                    }).collect::<Html>() }
                                                </div>
                                                { if has_overview {
                                                    html! {
                                                        <button class="monuli-back-button monuli-overview-btn" onmousedown={on_show_all}>
                                                            {"NÄYTÄ KAIKKI"}
                                                        </button>
                                                    }
                                                } else { html! {} } }
                                                <div class="monuli-word-list">
                                                    { word_order.iter().enumerate().map(|(pos, &word_index)| {
                                                        let (compact, extras) = monuli.compact_row(word_index);
                                                        let is_first_solved = pos == first_solved && first_solved < word_order.len();
                                                        let onselect = link.callback(move |e: MouseEvent| {
                                                            e.prevent_default();
                                                            Msg::SelectMonuliWord(Some(word_index))
                                                        });
                                                        let render_cell = |cell: &CompactTile| -> Html {
                                                            match cell {
                                                                CompactTile::Empty => html! {
                                                                    <div class="compact-cell"></div>
                                                                },
                                                                CompactTile::Absent(c) => html! {
                                                                    <div class="compact-cell absent">{ c }</div>
                                                                },
                                                                CompactTile::Correct(c) => html! {
                                                                    <div class="compact-cell correct">{ c }</div>
                                                                },
                                                                CompactTile::Yellow(c) => html! {
                                                                    <div class="compact-cell present">{ c }</div>
                                                                },
                                                                CompactTile::Brown(c) => html! {
                                                                    <div class="compact-cell maybe-present">{ c }</div>
                                                                },
                                                                CompactTile::Multi(ys, bs, aas) => html! {
                                                                    <div class="compact-cell compact-cell-multi">
                                                                        {
                                                                            ys.iter().map(|&c| html! {
                                                                                <span class="present">{ c }</span>
                                                                            }).chain(bs.iter().map(|&c| html! {
                                                                                <span class="maybe-present">{ c }</span>
                                                                            })).chain(aas.iter().map(|&c| html! {
                                                                                <span class="absent">{ c }</span>
                                                                            })).collect::<Html>()
                                                                        }
                                                                    </div>
                                                                },
                                                            }
                                                        };
                                                        let is_cursor = list_cursor == Some(pos);
                                                        let row_class = if is_cursor {
                                                            format!("row-{} monuli-compact-row monuli-row-selected", word_length)
                                                        } else {
                                                            format!("row-{} monuli-compact-row", word_length)
                                                        };
                                                        html! {
                                                            <>
                                                                { if is_first_solved {
                                                                    html! { <div class="monuli-separator">{"Ratkaistut sanulit"}</div> }
                                                                } else { html! {} } }
                                                                <div class={row_class}
                                                                     onmousedown={onselect}>
                                                                    <div class="compact-cells-side"></div>
                                                                    <div class="compact-cells-main">
                                                                        { compact.iter().map(&render_cell).collect::<Html>() }
                                                                    </div>
                                                                    <div class="compact-cells-side">
                                                                        { if extras.is_empty() {
                                                                            html! {}
                                                                        } else if extras.len() == 1 {
                                                                            let c = extras[0];
                                                                            html! {
                                                                                <div class="compact-cell present">{ c }</div>
                                                                            }
                                                                        } else {
                                                                            html! {
                                                                                <div class="compact-cell compact-cell-multi">
                                                                                    { extras.iter().map(|&c| html! {
                                                                                        <span class="present">{ c }</span>
                                                                                    }).collect::<Html>()
                                                                                    }
                                                                                </div>
                                                                            }
                                                                        } }
                                                                    </div>
                                                                    { if is_cursor {
                                                                        html! { <div class="monuli-row-arrow">{"→"}</div> }
                                                                    } else { html! {} } }
                                                                </div>
                                                            </>
                                                        }
                                                    }).collect::<Html>() }
                                                </div>
                                            </div>
                                        }
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
                        monuli_word_solved={
                            game.monuli_selected_word()
                                .map(|idx| game.monuli_word_is_solved(idx))
                                .unwrap_or(false)
                        }
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
                />
            }
        }
    }
}

/// Scroll `.monuli-word-list` so that the row at `target_pos` is centered.
fn scroll_monuli_word_list_to(target_pos: usize) {
    let window = match window() {
        Some(w) => w,
        None => return,
    };
    let document = match window.document() {
        Some(d) => d,
        None => return,
    };
    if let Some(list_el) = document.query_selector(".monuli-word-list").ok().flatten() {
        let children = list_el.children();
        if let Some(target_el) = children.item(target_pos as u32) {
            let container_height = list_el.client_height() as f64;
            let el = target_el.dyn_ref::<web_sys::HtmlElement>().unwrap();
            let row_top = el.offset_top() as f64 - list_el.dyn_ref::<web_sys::HtmlElement>().unwrap().offset_top() as f64;
            let row_height = el.offset_height() as f64;
            let scroll_to = (row_top - container_height / 2.0 + row_height / 2.0).max(0.0);
            list_el.set_scroll_top(scroll_to as i32);
        }
    }
}

/// Ensure `.monuli-word-list` scroll keeps cursor visible with margins:
/// scroll down if cursor goes past the 8th visible row, up if above the 3rd.
fn ensure_cursor_visible_in_list(cursor_pos: usize) {
    let window = match window() {
        Some(w) => w,
        None => return,
    };
    let document = match window.document() {
        Some(d) => d,
        None => return,
    };
    if let Some(list_el) = document.query_selector(".monuli-word-list").ok().flatten() {
        let children = list_el.children();
        if let Some(target_el) = children.item(cursor_pos as u32) {
            let list_html = list_el.dyn_ref::<web_sys::HtmlElement>().unwrap();
            let el = target_el.dyn_ref::<web_sys::HtmlElement>().unwrap();
            let row_top = el.offset_top() as f64 - list_html.offset_top() as f64;
            let row_height = el.offset_height() as f64;
            let scroll_top = list_el.scroll_top() as f64;
            let container_height = list_el.client_height() as f64;

            let row_bottom = row_top + row_height;
            let visible_top = scroll_top;
            let visible_bottom = scroll_top + container_height;

            let margin_top_rows = 2.0 * row_height;
            let margin_bottom_rows = 2.0 * row_height;

            if row_top < visible_top + margin_top_rows {
                let new_scroll = (row_top - margin_top_rows).max(0.0);
                list_el.set_scroll_top(new_scroll as i32);
            } else if row_bottom > visible_bottom - margin_bottom_rows {
                let new_scroll = row_bottom + margin_bottom_rows - container_height;
                list_el.set_scroll_top(new_scroll as i32);
            }
        }
    }
}

/// Ensure the currently active row in Monuli single-word view is visible.
fn ensure_sanuli_current_visible() {
    let window = match window() {
        Some(w) => w,
        None => return,
    };
    let document = match window.document() {
        Some(d) => d,
        None => return,
    };
    if let Some(container_el) = document.query_selector(".monuli-sanuli-view").ok().flatten() {
        if let Some(current_tile) = document.query_selector(".monuli-sanuli-view .current").ok().flatten() {
            if let Some(row_el) = current_tile.parent_element() {
                let container = container_el.dyn_ref::<web_sys::HtmlElement>().unwrap();
                let row = row_el.dyn_ref::<web_sys::HtmlElement>().unwrap();

                let row_top = row.offset_top() as f64 - container.offset_top() as f64;
                let row_height = row.offset_height() as f64;
                let scroll_top = container.scroll_top() as f64;
                let container_height = container.client_height() as f64;

                let row_bottom = row_top + row_height;
                let visible_top = scroll_top;
                let visible_bottom = scroll_top + container_height;

                let margin = row_height * 0.5;

                if row_top < visible_top + margin {
                    let new_scroll = (row_top - margin).max(0.0);
                    container.set_scroll_top(new_scroll as i32);
                } else if row_bottom > visible_bottom - margin {
                    let new_scroll = row_bottom + margin - container_height;
                    container.set_scroll_top(new_scroll as i32);
                }
            }
        }
    }
}

/// Set up hover highlighting on the overview grid via direct DOM manipulation.
/// Runs every render; uses a data attribute to avoid re-attaching listeners.
fn setup_overview_hover() {
    let document = match window().and_then(|w| w.document()) {
        Some(d) => d,
        None => return,
    };
    let grid = match document.query_selector(".monuli-overview-grid").ok().flatten() {
        Some(g) => g,
        None => return,
    };
    if grid.get_attribute("data-hover-init").is_some() {
        return;
    }
    grid.set_attribute("data-hover-init", "1").ok();

    let grid_el = grid.clone();
    let on_enter: Closure<dyn Fn(web_sys::MouseEvent)> = Closure::new(move |e: web_sys::MouseEvent| {
        let target = match e.target() {
            Some(t) => t,
            None => return,
        };
        let row_el = match target.dyn_ref::<web_sys::Element>()
            .and_then(|el| el.closest(".overview-row").ok().flatten()) {
            Some(r) => r,
            None => return,
        };
        let wo: usize = match row_el.get_attribute("data-wo").and_then(|s| s.parse().ok()) {
            Some(v) => v,
            None => return,
        };
        let k: usize = grid_el.get_attribute("data-k").and_then(|s| s.parse().ok()).unwrap_or(1);
        let cols: usize = grid_el.get_attribute("data-cols").and_then(|s| s.parse().ok()).unwrap_or(4);
        let n: usize = grid_el.get_attribute("data-n").and_then(|s| s.parse().ok()).unwrap_or(0);

        let hover_col = wo / k;
        let hover_row_in_col = wo % k;
        let col_len = if hover_col < cols - 1 || n % k == 0 { k } else { n % k };

        clear_overview_highlights(&grid_el);
        for offset in 0..=4usize {
            let up = (hover_row_in_col + col_len - (offset % col_len)) % col_len;
            let down = (hover_row_in_col + offset) % col_len;
            for idx in [hover_col * k + up, hover_col * k + down] {
                let selector = format!(".overview-row[data-wo=\"{}\"]", idx);
                if let Some(el) = grid_el.query_selector(&selector).ok().flatten() {
                    add_css_class(&el, "overview-row-highlight");
                }
            }
        }
        add_css_class(&row_el, "overview-row-hovered");
    });

    let grid_el2 = grid.clone();
    let on_leave: Closure<dyn Fn(web_sys::MouseEvent)> = Closure::new(move |_: web_sys::MouseEvent| {
        clear_overview_highlights(&grid_el2);
    });

    grid.add_event_listener_with_callback("mouseover", on_enter.as_ref().unchecked_ref()).ok();
    grid.add_event_listener_with_callback("mouseleave", on_leave.as_ref().unchecked_ref()).ok();
    on_enter.forget();
    on_leave.forget();
}

fn add_css_class(el: &web_sys::Element, class: &str) {
    let current = el.get_attribute("class").unwrap_or_default();
    if !current.split_whitespace().any(|c| c == class) {
        el.set_attribute("class", &format!("{} {}", current, class)).ok();
    }
}

fn remove_css_class(el: &web_sys::Element, class: &str) {
    if let Some(current) = el.get_attribute("class") {
        let new: String = current.split_whitespace()
            .filter(|c| *c != class)
            .collect::<Vec<_>>()
            .join(" ");
        el.set_attribute("class", &new).ok();
    }
}

fn clear_overview_highlights(grid: &web_sys::Element) {
    let children = grid.children();
    for i in 0..children.length() {
        if let Some(child) = children.item(i) {
            remove_css_class(&child, "overview-row-highlight");
            remove_css_class(&child, "overview-row-hovered");
        }
    }
}

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<App>::new().render();
}
