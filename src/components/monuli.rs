use itertools::Itertools;
use yew::prelude::*;
use gloo_events::EventListener;
use gloo_storage::{LocalStorage, Storage};
use web_sys::window;
use wasm_bindgen::JsCast;

use crate::Msg;
use crate::manager::EnterButton;
use crate::monuli::{Monuli, CompactTile};
use crate::game::Game;
use crate::components::board::Board;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub game: Monuli,
    pub callback: Callback<Msg>,
}

#[function_component(MonuliView)]
pub fn monuli_view(props: &Props) -> Html {
    let monuli = &props.game;
    let word_length = monuli.word_length();

    let auto_sort = use_state(|| {
        LocalStorage::get::<bool>("monuli_auto_sort").unwrap_or(true)
    });

    let list_cursor = use_state(|| None::<usize>);
    let selected_word_index = use_state(|| None::<usize>);

    // Handle key events for list navigation
    {
        let monuli = monuli.clone();
        let list_cursor_clone = list_cursor.clone();
        let selected_word_index_clone = selected_word_index.clone();
        use_effect_with(
            (list_cursor.clone(), (*selected_word_index).clone(), *auto_sort),
            move |&(ref list_cursor_handle, selected_val, auto_sort_val)| {
                let list_cursor_val = **list_cursor_handle;

                let listener = EventListener::new(&window().unwrap(), "keydown", move |event| {
                    if selected_val.is_some() {
                        return;
                    }

                    let event = event.dyn_ref::<web_sys::KeyboardEvent>().unwrap();
                    let key = event.key();

                    let mut handled = true;
                    match key.as_str() {
                        "ArrowUp" => {
                            let order = monuli.word_order(auto_sort_val);
                            let n = if !monuli.is_guessing() {
                                order.len()
                            } else {
                                order.iter().filter(|&&i| !monuli.word_is_solved(i)).count()
                            };
                            if n > 0 {
                                let new_val = match list_cursor_val {
                                    None => Some(n - 1),
                                    Some(0) => Some(n - 1),
                                    Some(c) => Some((c - 1).min(n - 1)),
                                };
                                list_cursor_clone.set(new_val);
                            }
                        }
                        "ArrowDown" => {
                            let order = monuli.word_order(auto_sort_val);
                            let n = if !monuli.is_guessing() {
                                order.len()
                            } else {
                                order.iter().filter(|&&i| !monuli.word_is_solved(i)).count()
                            };
                            if n > 0 {
                                let new_val = match list_cursor_val {
                                    None => Some(0),
                                    Some(c) if c >= n - 1 => Some(0),
                                    Some(c) => Some(c + 1),
                                };
                                list_cursor_clone.set(new_val);
                            }
                        }
                        "ArrowRight" => {
                            if let Some(cursor) = list_cursor_val {
                                let order = monuli.word_order(auto_sort_val);
                                if let Some(&word_index) = order.get(cursor) {
                                    selected_word_index_clone.set(Some(word_index));
                                }
                            }
                        }
                        "Enter" => {
                            // other cases are handled in main.rs:update
                            match monuli.enter_button_state() {
                                EnterButton::MonuliReturnToListView => {
                                    selected_word_index_clone.set(None);
                                }
                                _ => {}
                            }
                        }
                        _ => {
                            handled = false;
                        }
                    }
                    if handled {
                        event.prevent_default();
                        event.stop_propagation();
                    }
                });
                move || drop(listener)
            },
        );
    }

    // Scroll handling
    use_effect_with((selected_word_index.clone(), list_cursor.clone()), move |(selected, cursor)| {
        if selected.is_some() {
            // When entering Sanuli view, scroll to bottom if many guesses
            if let Some(board) = window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id("game-board"))
            {
                board.set_scroll_top(board.scroll_height());
            }
        } else if cursor.is_some() {
            // Ensure the cursor row is visible in list view
            if let Some(row) = window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_elements_by_class_name("monuli-row-selected").item(0))
            {
                row.scroll_into_view_with_bool(false);
            }
        }
    });

    if let Some(word_idx) = *selected_word_index {
        // Sanuli view for a single word
        if let Some(game_board) = monuli.board_for_word(word_idx) {
            let onmousedown = Callback::from(move |e: MouseEvent| {
                e.prevent_default();
                selected_word_index.set(None);
            });
            html! {
                <>
                    <div class="monuli-back-bar">
                        <button class="monuli-back-button" {onmousedown}>
                            {"← TAKAISIN"}
                        </button>
                    </div>
                    <div class="board-container monuli-sanuli-view">
                        <Board
                            guesses={game_board.guesses}
                            is_guessing={game_board.is_guessing}
                            current_guess={game_board.current_guess}
                            is_reset={false}
                            is_hidden={false}
                            previous_guesses={vec![]}
                            max_guesses={monuli.max_guesses()}
                            word_length={word_length}
                            board_class={"board-monuli".to_string()}
                        />
                    </div>
                </>
            }
        } else {
            html! {}
        }
    } else {
        // List view: single column, scrollable
        let word_order = monuli.word_order(*auto_sort);
        let current_letters: Vec<char> = monuli.last_guess().chars().collect();
        let mut first_solved = word_order.len();
        for (pos, &idx) in word_order.iter().enumerate() {
            if monuli.word_is_solved(idx) {
                first_solved = pos;
                break;
            }
        }

        let on_toggle_sort = {
            let auto_sort = auto_sort.clone();
            Callback::from(move |e: MouseEvent| {
                e.prevent_default();
                let new_val = !*auto_sort;
                auto_sort.set(new_val);
                let _ = LocalStorage::set("monuli_auto_sort", new_val);
            })
        };

        html! {
            <div class="monuli-list-view">
                <div class="monuli-list-header">
                    <div class="monuli-stats">
                        <div class="monuli-stat-item">
                            <span class="label">{"Yritys"}</span>
                            <span class="value">{ format!("{}/{}", if monuli.is_guessing() { monuli.current_guess + 1 } else { monuli.current_guess }, monuli.max_guesses()) }</span>
                        </div>
                        { if monuli.best_score > 0 {
                            html! {
                                <div class="monuli-stat-item">
                                    <span class="label">{"Paras"}</span>
                                    <span class="value">{ monuli.best_score }</span>
                                </div>
                            }
                        } else { html! {} } }
                        { if monuli.streak > 0 {
                            html! {
                                <div class="monuli-stat-item">
                                    <span class="label">{"Putki"}</span>
                                    <span class="value">{ monuli.streak }</span>
                                </div>
                            }
                        } else { html! {} } }
                    </div>
                    <button class={classes!("monuli-sort-toggle", if *auto_sort { "active" } else { "" })}
                            onmousedown={on_toggle_sort}>
                        { if *auto_sort { "Lajittelu: Päällä" } else { "Lajittelu: Pois" } }
                    </button>
                </div>
                <div class={format!("row-{}", word_length)}>
                    { (0..word_length).map(|i| {
                        let c = current_letters.get(i).copied().unwrap_or(' ');
                        html! { <div class={classes!("tile", "current", "unknown")}>{ c }</div> }
                    }).collect::<Html>() }
                </div>
                <div class="monuli-word-list">
                    { word_order.iter().enumerate().map(|(pos, &word_index)| {
                        let (compact, extras) = monuli.compact_row(word_index);
                        let is_first_solved = pos == first_solved && first_solved < word_order.len();
                        let selected_word_index = selected_word_index.clone();
                        let onselect = Callback::from(move |e: MouseEvent| {
                            e.prevent_default();
                            selected_word_index.set(Some(word_index));
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
                                CompactTile::Present(c) => html! {
                                    <div class="compact-cell present">{ c }</div>
                                },
                                CompactTile::MaybePresent(c) => html! {
                                    <div class="compact-cell maybe-present">{ c }</div>
                                },
                                CompactTile::Multi(ps, mps, aas) => html! {
                                    <div class="compact-cell compact-cell-multi">
                                        {
                                            ps.iter().sorted().map(|&c| html! {
                                                <span class="present">{ c }</span>
                                            }).chain(mps.iter().sorted().map(|&c| html! {
                                                <span class="maybe-present">{ c }</span>
                                            })).chain(aas.iter().sorted().map(|&c| html! {
                                                <span class="absent">{ c }</span>
                                            })).collect::<Html>()
                                        }
                                    </div>
                                },
                            }
                        };
                        let is_cursor = *list_cursor == Some(pos);
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
                                        { if extras.len() == 0 {
                                            html! {}
                                        } else if extras.len() == 1 {
                                            let c = extras.iter().next().unwrap();
                                            html! {
                                                <div class="compact-cell present">{ c }</div>
                                            }
                                        } else {
                                            html! {
                                                <div class="compact-cell compact-cell-multi">
                                                    { extras.iter().sorted().map(|&c| html! {
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
}
