use rand::Rng;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use futures::{future, Future, IntoFuture};
use comms::Broadcasting;

use super::*;
use net::*;
use utils::*;

pub fn game_future<R>(mut game: Game<R>,
                      players: Room,
                      spectators_ref: Arc<Mutex<Room>>,
                      timeout: Option<Milliseconds>)
                      -> <GameFuture<R> as IntoFuture>::Future
    where R: Rng + 'static
{
    for name in players.client_names() {
        if let Some(name) = name {
            game.add_player(name.clone());
        }
    }

    GameFuture {
            game: game,
            players: players,
            spectators_ref: spectators_ref,
            timeout: timeout.map(|m| *m),
        }
        .into_future()
}

type BoxedFuture<I, E> = Box<Future<Item = I, Error = E>>;

pub struct GameFuture<R>
    where R: Rng + 'static
{
    game: Game<R>,
    players: Room,
    spectators_ref: Arc<Mutex<Room>>,
    timeout: Option<Duration>,
}

impl<R> GameFuture<R>
    where R: Rng + 'static
{
    fn game_tx(self) -> BoxedFuture<Self, ()> {
        let game_msg = Msg::Game { game: Box::new(self.game.game_state().clone()) };
        let spectators_ref2 = self.spectators_ref.clone();
        let spectators = spectators_ref2.lock().unwrap();
        // N.B. Clones Room and associated Clients. Expensive.
        let b = (self.players.clone(), spectators.clone()).broadcast(game_msg);
        Box::new(b.map(|_| self))
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

    fn round_tx(self) -> BoxedFuture<Self, ()> {
        let round_msg = Msg::Round {
            round: Box::new(self.game.round_state().clone()),
            game_uuid: self.game.game_state().uuid,
        };
        let spectators_ref2 = self.spectators_ref.clone();
        let spectators = spectators_ref2.lock().unwrap();
        // N.B. Clones Room and associated Clients. Expensive.
        Box::new((self.players.clone(), spectators.clone()).broadcast(round_msg).map(|_| self))
    }

    fn move_rx(self) -> BoxedFuture<(Self, HashMap<ClientId, Msg>), ()> {
        let receive_timeout = ClientTimeout::keep_alive_after(self.timeout);
        Box::new(self.players
            .filter(|client| self.game.round_state().snakes.contains_key(&client.name().unwrap()))
            .receive(receive_timeout)
            .map(|(_, msgs)| (self, msgs)))
    }

    fn perform_move(&mut self, msgs: HashMap<ClientId, Msg>) {
        let directions = self.msgs_to_directions(msgs);
        self.game.next(Event::Turn(directions));
    }

    fn outcome_tx(self) -> BoxedFuture<Self, ()> {
        let outcome_msg = Msg::outcome(self.game.round_state().clone(),
                                       self.game.game_state().uuid);
        let spectators_ref2 = self.spectators_ref.clone();
        let spectators = spectators_ref2.lock().unwrap();
        // N.B. Clones Room and associated Clients. Expensive.
        Box::new((self.players.clone(), spectators.clone()).broadcast(outcome_msg).map(|_| self))
    }

    fn msgs_to_directions(&self, msgs: HashMap<ClientId, Msg>) -> HashMap<String, Direction> {
        msgs.into_iter()
            .filter_map(|(id, msg)| if let Msg::Move { direction } = msg {
                let name = self.players.name_of(&id).unwrap();
                Some((name, direction))
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
    type Item = (Game<R>, Room, Arc<Mutex<Room>>);
    type Error = ();

    fn into_future(self) -> Self::Future {
        Box::new(self.game_tx()
            .and_then(Self::round_loop)
            .and_then(Self::outcome_tx)
            .map(|self_| (self_.game, self_.players, self_.spectators_ref)))
    }
}
