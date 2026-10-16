//! Untimed local practice with a small deterministic computer opponent.

use crate::board::{GameBoard, PieceImage};
use crate::sound;
use chess_core::chess::{Color, Game, GameStatus, Move, PieceKind, Square};
use dioxus::prelude::*;

fn score(game: &Game) -> i32 {
    match game.status() {
        GameStatus::Checkmate {
            winner: Color::White,
        } => 1000,
        GameStatus::Checkmate {
            winner: Color::Black,
        } => -1000,
        status if status.is_over() => 0,
        _ => (0..64)
            .filter_map(|sq| game.board().piece_at(Square(sq)))
            .map(|piece| {
                piece.kind.material_value() as i32
                    * if piece.color == Color::White { 1 } else { -1 }
            })
            .sum(),
    }
}

fn computer_move(game: &Game) -> Option<Move> {
    if game.status().is_over() || game.side_to_move() != Color::Black {
        return None;
    }
    // shortcut: two-ply material search is a beginner opponent; use an engine for stronger play.
    game.board().legal_moves().into_iter().min_by_key(|mv| {
        let mut next = game.clone();
        next.play(*mv).expect("generated move is legal");
        if next.status().is_over() {
            return score(&next);
        }
        next.board()
            .legal_moves()
            .into_iter()
            .map(|reply| {
                let mut response = next.clone();
                response.play(reply).expect("generated reply is legal");
                score(&response)
            })
            .max()
            .unwrap_or_else(|| score(&next))
    })
}

fn play_move(mut game: Signal<Game>, mv: Move) {
    let clip = game.with_mut(|game| {
        let outcome = game.play(mv).ok()?.clone();
        Some(sound::for_move(&outcome, game.status().is_over()))
    });
    if let Some(clip) = clip {
        sound::play(clip);
    }
}

fn undo_turn(game: &Game) -> Game {
    let keep = game.ply().saturating_sub(1) / 2 * 2;
    Game::from_moves(&game.moves()[..keep]).expect("previous moves are legal")
}

#[component]
pub fn PracticeView() -> Element {
    let mut game = use_signal(Game::new);
    use_future(move || async move {
        loop {
            crate::gloo_sleep(180).await;
            if game.peek().side_to_move() != Color::Black || game.peek().status().is_over() {
                continue;
            }
            let position = game.peek().clone();
            if let Some(mv) = computer_move(&position) {
                play_move(game, mv);
            }
        }
    });
    let position = game.read();
    let status = position.status();
    let thinking = !status.is_over() && position.side_to_move() == Color::Black;
    let playable = (!status.is_over() && !thinking).then_some(Color::White);
    let fen = position.current_fen().to_string();
    let last_move = position.moves().last().copied();
    let sans = position.san_list();
    let can_undo = position.ply() > 0;
    let label = match status {
        GameStatus::InProgress if thinking => "Computer is thinking…",
        GameStatus::InProgress if position.board().is_check() => "Your turn — in check",
        GameStatus::InProgress => "Your turn",
        GameStatus::Checkmate {
            winner: Color::White,
        } => "You won by checkmate",
        GameStatus::Checkmate {
            winner: Color::Black,
        } => "Computer won by checkmate",
        GameStatus::Stalemate => "Draw by stalemate",
        GameStatus::InsufficientMaterial => "Draw by insufficient material",
        GameStatus::FiftyMoveRule => "Draw by fifty-move rule",
        GameStatus::ThreefoldRepetition => "Draw by repetition",
    };
    drop(position);
    rsx! {
        div { class: "page",
            div { class: "game-toolbar",
                h1 { "Play computer" }
                span { class: "spacer" }
                span { class: "tag", "Untimed" }
            }
            div { class: "game-layout",
                div {
                    div { class: "seat",
                        div { class: "seat-avatar", PieceImage { kind: PieceKind::King, color: Color::Black } }
                        span { class: "name", "Computer" }
                        span { class: "spacer" }
                        span { class: "muted small", "Beginner" }
                    }
                    div { class: "board-wrap",
                        GameBoard { fen, orientation: Color::White, playable, last_move,
                            on_move: move |mv| {
                                if game.peek().side_to_move() == Color::White {
                                    play_move(game, mv);
                                }
                            }
                        }
                    }
                    div { class: "seat",
                        div { class: "seat-avatar", PieceImage { kind: PieceKind::King, color: Color::White } }
                        span { class: "name", "You" }
                        span { class: "spacer" }
                        span { class: "muted small", "White" }
                    }
                }
                div { class: "panel",
                    h2 { role: "status", aria_live: "polite", "{label}" }
                    p { class: "muted small", "Select a piece, then its destination. Dragging and keyboard controls work too." }
                    div { class: "replay-bar",
                        button { class: "primary", onclick: move |_| game.set(Game::new()), "New game" }
                        button { disabled: !can_undo, onclick: move |_| {
                            let restored = undo_turn(&game.peek());
                            game.set(restored);
                        }, "Undo turn" }
                    }
                    h2 { "Moves" }
                    div { class: "moves",
                        if sans.is_empty() { div { class: "empty", "Play your first move." } }
                        for (i, pair) in sans.chunks(2).enumerate() {
                            div { class: "mv-row",
                                span { class: "num", "{i + 1}." }
                                span { class: "mv", "{pair[0]}" }
                                if let Some(reply) = pair.get(1) {
                                    span { class: "mv", "{reply}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chess_core::chess::Board;

    #[test]
    fn computer_takes_free_material_and_finds_mate() {
        let capture = Game::from_board(Board::from_fen("r6k/8/8/8/Q7/8/8/7K b - - 0 1").unwrap());
        assert_eq!(computer_move(&capture).unwrap().to_uci(), "a8a4");
        let mut mate = Game::from_board(Board::from_fen("8/8/8/8/8/6q1/5k2/7K b - - 0 1").unwrap());
        let mv = computer_move(&mate).unwrap();
        assert!(mate.is_legal(mv));
        mate.play(mv).unwrap();
        assert_eq!(
            mate.status(),
            GameStatus::Checkmate {
                winner: Color::Black
            }
        );
        assert!(computer_move(&mate).is_none());
    }

    #[test]
    fn computer_waits_for_white_and_never_moves_after_stalemate() {
        assert!(computer_move(&Game::new()).is_none());
        let draw = Game::from_board(Board::from_fen("k7/2Q5/8/8/8/8/8/4K3 b - - 0 1").unwrap());
        assert_eq!(draw.status(), GameStatus::Stalemate);
        assert!(computer_move(&draw).is_none());
    }

    #[test]
    fn undo_returns_to_humans_previous_turn_even_while_computer_waits() {
        let mut game = Game::new();
        game.play(Move::from_uci("e2e4").unwrap()).unwrap();
        assert_eq!(undo_turn(&game).ply(), 0);
        game.play(Move::from_uci("e7e5").unwrap()).unwrap();
        assert_eq!(undo_turn(&game).ply(), 0);
        game.play(Move::from_uci("g1f3").unwrap()).unwrap();
        let restored = undo_turn(&game);
        assert_eq!(restored.ply(), 2);
        assert_eq!(restored.side_to_move(), Color::White);
    }
}
