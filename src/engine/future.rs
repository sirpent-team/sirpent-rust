use rand::Rng;
use std::time::Duration;
use std::collections::HashMap;
use futures::{future, Future, Sink};
use futures::sync::mpsc;
use tokio_timer::Timer;

use super::*;
use net::*;
use utils::*;

pub fn game_future<R>
    (mut game: Game<R>,
     players: MsgRoom<String>,
     spectator_tx: mpsc::Sender<Msg>,
     timeout: Option<Milliseconds>,
     timer: Timer)
     -> Box<Future<Item = (Game<R>, MsgRoom<String>, mpsc::Sender<Msg>), Error = ()>>
    where R: Rng + 'static
{
    let timeout = *timeout.unwrap();

    for id in players.ids() {
        game.add_player(id.clone());
    }

    let game_msg = Msg::Game { game: Box::new(game.game_state().clone()) };
    let players_game_future = players.broadcast_all(game_msg.clone());
    let spectator_tx_game_future = spectator_tx.send(game_msg).map_err(|_| ());

    let rounds_future_fn =
        move |(players, spectator_tx)| rounds(game, players, spectator_tx, timeout, timer);

    let outcome_fn =
        |(game, players, spectator_tx): (Game<R>, MsgRoom<String>, mpsc::Sender<Msg>)| {
            let outcome_msg = Msg::outcome(game.round_state().clone(), game.game_state().uuid);
            let players_outcome_future = players.broadcast_all(outcome_msg.clone());
            let spectator_tx_outcome_future = spectator_tx.send(outcome_msg).map_err(|_| ());

            players_outcome_future
                .join(spectator_tx_outcome_future)
                .map(|(players, spectator_tx)| (game, players, spectator_tx))
        };

    Box::new(players_game_future
                 .join(spectator_tx_game_future)
                 .and_then(rounds_future_fn)
                 .and_then(outcome_fn))
}

fn rounds<R>(game: Game<R>,
             players: MsgRoom<String>,
             spectator_tx: mpsc::Sender<Msg>,
             timeout: Duration,
             timer: Timer)
             -> Box<Future<Item = (Game<R>, MsgRoom<String>, mpsc::Sender<Msg>), Error = ()>>
    where R: Rng + 'static
{
    let round_fn = move |(game, players, spectator_tx)| {
        round(game, players, spectator_tx, timeout, timer.clone()).map(|(game, players, spectator_tx)| {
            if game.concluded() {
                future::Loop::Break((game, players, spectator_tx))
            } else {
                future::Loop::Continue((game, players, spectator_tx))
            }
        })
    };
    let loop_future = future::loop_fn((game, players, spectator_tx), round_fn);
    Box::new(loop_future)
}

fn round<R>(mut game: Game<R>,
            players: MsgRoom<String>,
            spectator_tx: mpsc::Sender<Msg>,
            _: Duration,
            _: Timer)
            -> Box<Future<Item = (Game<R>, MsgRoom<String>, mpsc::Sender<Msg>), Error = ()>>
    where R: Rng + 'static
{
    let round_msg = Msg::Round {
        round: Box::new(game.round_state().clone()),
        game_uuid: game.game_state().uuid,
    };
    let players_round_future = players.broadcast_all(round_msg.clone());
    let spectator_tx_round_future = spectator_tx.send(round_msg).map_err(|_| ());

    let move_future_fn = |(players, spectator_tx): (MsgRoom<String>, _)| {
        let living_player_ids = game.round_state().snakes.keys().cloned().collect();
        players
            .receive(living_player_ids)
            //.with_soft_timeout(timeout, &timer)
            .map(|(msgs, players)| {
                     let directions = msgs_to_directions(msgs);
                     game.next(Event::Turn(directions));
                     (game, players, spectator_tx)
                 })
    };

    Box::new(players_round_future
                 .join(spectator_tx_round_future)
                 .and_then(move_future_fn))
}

fn msgs_to_directions(msgs: HashMap<String, Msg>) -> HashMap<String, Direction> {
    msgs.into_iter()
        .filter_map(|(id, msg)| if let Msg::Move { direction } = msg {
                        Some((id, direction))
                    } else {
                        None
                    })
        .collect()
}
