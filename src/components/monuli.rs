use itertools::Itertools;
use yew::prelude::*;
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
    let link = &props.callback;
    let word_length = monuli.word_length();

    if let Some(word_idx) = monuli.selected_word_index {
        // Sanuli view for a single word
        if let Some(game_board) = monuli.board_for_word(word_idx) {
            let onback = link.clone();
            let onback_cb = Callback::from(move |e: MouseEvent| {
                e.prevent_default();
                onback.emit(Msg::SelectMonuliWord(None));
            });
            html! {
                <>
                    <div class="monuli-back-bar">
                        <button class="monuli-back-button" onmousedown={onback_cb}>
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
        let word_order = monuli.word_order();
        let current_letters: Vec<char> = monuli.last_guess().chars().collect();
        let mut first_solved = word_order.len();
        for (pos, &idx) in word_order.iter().enumerate() {
            if monuli.word_is_solved(idx) {
                first_solved = pos;
                break;
            }
        }

        let list_cursor = monuli.list_cursor;

        html! {
            <div class="monuli-list-view">
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
                        let callback = link.clone();
                        let onselect = Callback::from(move |e: MouseEvent| {
                            e.prevent_default();
                            callback.emit(Msg::SelectMonuliWord(Some(word_index)));
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
