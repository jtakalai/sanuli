use itertools::Itertools;
use yew::prelude::*;
use gloo_events::EventListener;
use gloo_storage::{LocalStorage, Storage};
use web_sys::window;
use wasm_bindgen::JsCast;

use crate::Msg;
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

    // Handle key events for list navigation
    {
        let monuli_clone = monuli.clone();
        let list_cursor_clone = list_cursor.clone();
        let callback = props.callback.clone();

        use_effect_with(
            (list_cursor.clone(), *auto_sort, monuli.selected_word_index, monuli.current_guess),
            move |&(ref list_cursor_handle, auto_sort_val, _selected_word_idx, _current_guess)| {
                let list_cursor_val = **list_cursor_handle;

                let listener = EventListener::new(&window().unwrap(), "keydown", move |event| {
                    let event = event.dyn_ref::<web_sys::KeyboardEvent>().unwrap();
                    let key = event.key();
                    let key = key.as_str();

                    let mut handled = true;
                    if monuli_clone.selected_word_index.is_none() {
                        // list view
                        if key == "ArrowUp" {
                            let order = monuli_clone.word_order(auto_sort_val);
                            let n = if !monuli_clone.is_guessing() {
                                order.len()
                            } else {
                                order.iter().filter(|&&i| !monuli_clone.word_is_solved(i)).count()
                            };
                            if n > 0 {
                                let new_val = match list_cursor_val {
                                    None => Some(n - 1),
                                    Some(0) => Some(n - 1),
                                    Some(c) => Some((c - 1).min(n - 1)),
                                };
                                list_cursor_clone.set(new_val);
                            }
                        } else if key == "ArrowDown" {
                            let order = monuli_clone.word_order(auto_sort_val);
                            let n = if !monuli_clone.is_guessing() {
                                order.len()
                            } else {
                                order.iter().filter(|&&i| !monuli_clone.word_is_solved(i)).count()
                            };
                            if n > 0 {
                                let new_val = match list_cursor_val {
                                    None => Some(0),
                                    Some(c) if c >= n - 1 => Some(0),
                                    Some(c) => Some(c + 1),
                                };
                                list_cursor_clone.set(new_val);
                            }
                        } else if key == "ArrowRight" {
                            if monuli_clone.selected_word_index.is_none() {
                                if let Some(cursor) = list_cursor_val {
                                    let order = monuli_clone.word_order(auto_sort_val);
                                    if let Some(&word_index) = order.get(cursor) {
                                        callback.emit(Msg::SetMonuliSelection(Some(word_index)));
                                    }
                                }
                            }
                        } else {
                            handled = false;
                        }
                    } else {
                        // individual sanuli view
                        if key == "ArrowLeft" {
                            callback.emit(Msg::SetMonuliSelection(None));
                        } else {
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
    {
        let selected_word_index = monuli.selected_word_index;
        use_effect_with(list_cursor.clone(), move |cursor| {
            if selected_word_index.is_some() {
                // When entering Sanuli view, scroll to bottom if many guesses
                if let Some(board) = window()
                    .and_then(|w| w.document())
                    .and_then(|d| d.get_element_by_id("game-board"))
                {
                    board.set_scroll_top(board.scroll_height().into());
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
    }

    if let Some(word_idx) = monuli.selected_word_index {
        // Sanuli view for a single word
        if let Some(game_board) = monuli.board_for_word(word_idx) {
            let callback = props.callback.clone();
            let onmousedown = Callback::from(move |e: MouseEvent| {
                e.prevent_default();
                callback.emit(Msg::SetMonuliSelection(None));
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
                <div class="monuli-list-layout">
                    <div class="monuli-stats-sidebar">
                        <div class="monuli-stats">
                            <div class="monuli-stat-item">
                                <span class="label">{"Arvaus"}</span>
                                <span class="value">{ format!("{}/{}", if monuli.is_guessing() { monuli.current_guess + 1 } else { monuli.current_guess }, monuli.max_guesses()) }</span>
                            </div>
                            <div class="monuli-stat-item">
                                <span class="label">{"Paras tulos"}</span>
                                <span class="value">{ if monuli.best_score > 0 { monuli.best_score.to_string() } else { "-".to_string() } }</span>
                            </div>
                            <div class="monuli-stat-item">
                                <span class="label">{"Monuli- putki"}</span>
                                <span class="value">{ monuli.streak }</span>
                            </div>
                            <div class="monuli-stat-item">
                                <span class="label">{"Järjestä sanat"}</span>
                                <button class={classes!("monuli-sort-toggle", if *auto_sort { "active" } else { "" })}
                                        onmousedown={on_toggle_sort}>
                                    { if *auto_sort { "ON" } else { "OFF" } }
                                </button>
                            </div>
                        </div>
                    </div>
                    <div class="monuli-list-main">
                        <div class={format!("row-{} monuli-compact-row", word_length)} style="cursor: default;">
                            <div class="compact-cells-main">
                                { (0..word_length).map(|i| {
                                    let c = current_letters.get(i).copied().unwrap_or(' ');
                                    html! { <div class={classes!("tile", "current", "unknown")}>{ c }</div> }
                                }).collect::<Html>() }
                            </div>
                            <div class="compact-cells-side"></div>
                        </div>
                        <div class="monuli-word-list">
                            { word_order.iter().enumerate().map(|(pos, &word_index)| {
                                let (compact, extras) = monuli.compact_row(word_index);
                                let is_first_solved = pos == first_solved && first_solved < word_order.len();
                                let callback = props.callback.clone();
                                let onselect = Callback::from(move |e: MouseEvent| {
                                    e.prevent_default();
                                    callback.emit(Msg::SetMonuliSelection(Some(word_index)));
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
                </div>
            </div>
        }
    }
}
