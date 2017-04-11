use futures::{future, Future};
use tokio_timer;
use std::net::SocketAddr;
use state::GridEnum;
use kabuki::{Actor, ActorRef};
use std::fmt::Debug;
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
            //.with_soft_timeout(timeout, &timer)
            .map(|(msgs, players)| {
                (msgs_to_directions(msgs), players)
            });
        Box::new(future)
    }
}

impl Actor for GameActor {
    type Request = (Game<Box<Rng>>, MsgRoom<String>, Milliseconds);
    type Response = (Game<Box<Rng>>, MsgRoom<String>);
    type Error = ();
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&mut self, (game, players, timeout): Self::Request) -> Self::Future {
        let GameActor {
            timer,
            spectator_tx,
        } = self.clone();

        for id in players.ids() {
            game.add_player(id);
        }

        let game_msg = Msg::Game { game: Box::new(game.game_state().clone()) };
        let future = Self::broadcast(game_msg, players, spectator_tx)
            .and_then(Self::rounds)
            .and_then(Self::outcome);
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
