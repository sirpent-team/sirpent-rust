use rand::Rng;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use futures::{future, Future, IntoFuture};
use tokio_timer::Timer;

use super::*;
use net::*;
use utils::*;

pub fn game_future<R>(mut game: Game<R>,
                      players: Room<String>,
                      spectators_ref: Arc<Mutex<Room<String>>>,
                      timeout: Option<Milliseconds>,
                      timer: Timer)
                      -> <GameFuture<R> as IntoFuture>::Future
    where R: Rng + 'static
{
    for id in players.ids() {
        game.add_player(id.clone());
    }

    GameFuture {
            game: game,
            players: players,
            spectators_ref: spectators_ref,
            timeout: timeout.map(|m| *m),
            timer: timer,
        }
        .into_future()
}

type BoxedFuture<I, E> = Box<Future<Item = I, Error = E>>;

pub struct GameFuture<R>
    where R: Rng + 'static
{
    game: Game<R>,
    players: Room<String>,
    spectators_ref: Arc<Mutex<Room<String>>>,
    timeout: Option<Duration>,
    timer: Timer,
}

impl<R> GameFuture<R>
    where R: Rng + 'static
{
    fn game_tx(mut self) -> BoxedFuture<Self, ()> {
        let game_msg = Msg::Game { game: Box::new(self.game.game_state().clone()) };
        // N.B. Clones Room and associated Clients. Expensive.
        // Box::new(self.players
        //     .broadcast(game_msg.clone())
        //     .and_then(|players| {
        //         self.players = players;
        //         self.spectators_ref.clone().lock().unwrap().broadcast(game_msg).map(|_| {
        //             self.players = players;
        //             self
        //         })
        //     }))
        let spectators_ref = self.spectators_ref.clone();
        let spectators = spectators_ref.lock().unwrap();
        Box::new((self.players.broadcast(game_msg.clone()), spectators.broadcast(game_msg))
            .into_future()
            .map(|(players, _)| {
                self.players = players;
                self
            }))
    }

    fn round_loop(self) -> BoxedFuture<Self, ()> {
        Box::new(future::loop_fn(self, |self_| {
            self_.round_tx()
                .and_then(Self::move_rx)
                .map(|(mut self_, msgs)| {
                    self_.perform_move(msgs);
                    if self_.game.concluded() {
                        future::Loop::Break(self_)
                    } else {
                        future::Loop::Continue(self_)
                    }
                })
        }))
    }

    fn round_tx(mut self) -> BoxedFuture<Self, ()> {
        let round_msg = Msg::Round {
            round: Box::new(self.game.round_state().clone()),
            game_uuid: self.game.game_state().uuid,
        };
        let spectators_ref2 = self.spectators_ref.clone();
        let spectators = spectators_ref2.lock().unwrap();
        // N.B. Clones Room and associated Clients. Expensive.

        Box::new(self.players
            .broadcast(round_msg.clone())
            .and_then(|players| {
                spectators.broadcast(round_msg).map(|_| {
                    self.players = players;
                    self
                })
            }))
    }

    fn move_rx(mut self) -> BoxedFuture<(Self, HashMap<String, Msg>), ()> {
        let receive_timeout = ClientTimeout::keep_alive_after(self.timeout, &self.timer);
        Box::new(self.players
            .receive_from(self.game.round_state().snakes.keys().cloned().collect(),
                          receive_timeout)
            .map(|(msgs, players)| {
                self.players = players;
                (self, msgs)
            }))
    }

    fn perform_move(&mut self, msgs: HashMap<String, Msg>) {
        let directions = self.msgs_to_directions(msgs);
        self.game.next(Event::Turn(directions));
    }

    fn outcome_tx(mut self) -> BoxedFuture<Self, ()> {
        let outcome_msg = Msg::outcome(self.game.round_state().clone(),
                                       self.game.game_state().uuid);
        let spectators_ref2 = self.spectators_ref.clone();
        let spectators = spectators_ref2.lock().unwrap();
        // N.B. Clones Room and associated Clients. Expensive.
        Box::new(self.players
            .broadcast(outcome_msg.clone())
            .and_then(|players| {
                spectators.broadcast(outcome_msg).map(|_| {
                    self.players = players;
                    self
                })
            }))
    }

    fn msgs_to_directions(&self, msgs: HashMap<String, Msg>) -> HashMap<String, Direction> {
        msgs.into_iter()
            .filter_map(|(id, msg)| if let Msg::Move { direction } = msg {
                Some((id, direction))
            } else {
                None
            })
            .collect()
    }
}

impl<R> IntoFuture for GameFuture<R>
    where R: Rng + 'static
{
    type Future = BoxedFuture<Self::Item, Self::Error>;
    type Item = (Game<R>, Room<String>, Arc<Mutex<Room<String>>>);
    type Error = ();

    fn into_future(self) -> Self::Future {
        Box::new(self.game_tx()
            .and_then(Self::round_loop)
            .and_then(Self::outcome_tx)
            .map(|self_| (self_.game, self_.players, self_.spectators_ref)))
    }
}
