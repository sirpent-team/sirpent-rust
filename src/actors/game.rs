use futures::{future, Future};
use tokio_timer;
use kabuki::Actor;
use std::collections::{HashMap, HashSet};
use futures::sync::mpsc;
use rand::Rng;
use futures::Sink;

use net::*;
use state::*;
use engine::*;
use utils::*;

#[derive(Clone)]
pub struct GameActor {
    timer: tokio_timer::Timer,
    spectator_tx: mpsc::Sender<Msg>,
}

impl GameActor {
    pub fn new(timer: tokio_timer::Timer, spectator_tx: mpsc::Sender<Msg>) -> GameActor {
        GameActor {
            timer: timer,
            spectator_tx: spectator_tx,
        }
    }

    fn broadcast(msg: Msg,
                 players: MsgRoom<String>,
                 spectator_tx: mpsc::Sender<Msg>)
                 -> Box<Future<Item = (MsgRoom<String>, mpsc::Sender<Msg>), Error = ()>> {
        let tx_to_players = players.broadcast_all(msg.clone());
        let tx_to_spectators = spectator_tx.send(msg).map_err(|_| ());
        Box::new(tx_to_players.join(tx_to_spectators).map_err(|_| ()))
    }

    fn receive_move_direction
        (players: MsgRoom<String>,
         living_player_ids: HashSet<String>,
         timeout: Milliseconds,
         timer: &tokio_timer::Timer)
         -> Box<Future<Item = (HashMap<String, Direction>, MsgRoom<String>), Error = ()>> {
        let future = players
            .receive(living_player_ids)
            //.with_soft_timeout(timeout, timer) // @TODO
            .map(|(msgs, players)| {
                (msgs_to_directions(msgs), players)
            });
        Box::new(future)
    }

    fn rounds
        (game: Game<Box<Rng>>,
         players: MsgRoom<String>,
         spectator_tx: mpsc::Sender<Msg>,
         timeout: Milliseconds,
         timer: tokio_timer::Timer)
         -> Box<Future<Item = (Game<Box<Rng>>, MsgRoom<String>, mpsc::Sender<Msg>), Error = ()>> {
        let inputs = (game, players, spectator_tx, timeout, timer.clone());
        let future = future::loop_fn(inputs, |(a, b, c, d, e)| {
            Self::round(a, b, c, d, e).map(|ret| {
                let (game, players, spectator_tx, timeout, timer) = ret;
                if game.concluded() {
                    future::Loop::Break((game, players, spectator_tx))
                } else {
                    future::Loop::Continue((game, players, spectator_tx, timeout, timer))
                }
            })
        });
        Box::new(future)
    }

    fn round(mut game: Game<Box<Rng>>,
             players: MsgRoom<String>,
             spectator_tx: mpsc::Sender<Msg>,
             timeout: Milliseconds,
             timer: tokio_timer::Timer)
             -> Box<Future<Item = (Game<Box<Rng>>,
                                   MsgRoom<String>,
                                   mpsc::Sender<Msg>,
                                   Milliseconds,
                                   tokio_timer::Timer),
                           Error = ()>> {
        let round_msg = Msg::Round {
            round: Box::new(game.round_state().clone()),
            game_uuid: game.game_state().uuid,
        };
        let future = Self::broadcast(round_msg, players, spectator_tx)
            .and_then(move |(players, spectator_tx)| {
                let living_player_ids = game.round_state().snakes.keys().cloned().collect();
                Self::receive_move_direction(players, living_player_ids, timeout, &timer)
                    .map(move |(directions, players)| {
                             game.next(Event::Turn(directions));
                             (game, players, spectator_tx, timeout, timer)
                         })
            });
        Box::new(future)
    }

    fn outcome
        (game: Game<Box<Rng>>,
         players: MsgRoom<String>,
         spectator_tx: mpsc::Sender<Msg>)
         -> Box<Future<Item = (Game<Box<Rng>>, MsgRoom<String>, mpsc::Sender<Msg>), Error = ()>> {
        let outcome_msg = Msg::outcome(game.round_state().clone(), game.game_state().uuid);
        let future = Self::broadcast(outcome_msg, players, spectator_tx).map(|(players,
                                                                               spectator_tx)| {
                                                                                 (game,
                                                                                  players,
                                                                                  spectator_tx)
                                                                             });
        Box::new(future)
    }
}

impl Actor for GameActor {
    type Request = (Game<Box<Rng>>, MsgRoom<String>, Milliseconds);
    type Response = (Game<Box<Rng>>, MsgRoom<String>);
    type Error = ();
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&mut self, (mut game, players, timeout): Self::Request) -> Self::Future {
        let GameActor {
            timer,
            spectator_tx,
        } = self.clone();

        for id in players.ids() {
            game.add_player(id);
        }

        let game_msg = Msg::Game { game: Box::new(game.game_state().clone()) };
        let future = Self::broadcast(game_msg, players, spectator_tx)
            .and_then(move |(players, spectator_tx)| {
                          Self::rounds(game, players, spectator_tx, timeout, timer)
                      })
            .and_then(|(game, players, spectator_tx)| Self::outcome(game, players, spectator_tx))
            .map(|(game, players, _)| (game, players));
        Box::new(future)
    }
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
