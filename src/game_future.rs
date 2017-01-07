use std::io;
use std::vec;
use std::iter::FromIterator;
use std::net::SocketAddr;
use std::time::Duration;
use std::marker::Send;
use std::collections::{HashSet, HashMap};
use std::collections::hash_map::{Keys, Drain};
use std::fmt::Debug;
use serde_json;
use rand::Rng;

use futures::{Async, Future, BoxFuture, Stream, Sink, Poll};
use futures::stream::{SplitStream, SplitSink, futures_unordered};
use tokio_core::net::TcpStream;
use tokio_core::io::Io;
use tokio_core::reactor::Remote;

use engine::*;
use clients::*;
use protocol::*;

pub struct GameFuture<S, T, R>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send + 'static,
          T: Stream<Item = Msg, Error = io::Error> + Send + 'static,
          R: Rng
{
    game: Option<Engine<R>>,
    players: Option<Clients<S, T>>,
    current_stage: Option<GameFutureStage<S, T>>,
}

impl<S, T, R> GameFuture<S, T, R>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send + 'static,
          T: Stream<Item = Msg, Error = io::Error> + Send + 'static,
          R: Rng
{
    pub fn new(mut game: Engine<R>, players: Clients<S, T>) -> Self {
        for name in players.names() {
            game.add_player(name.clone());
        }

        GameFuture {
            game: Some(game),
            players: Some(players),
            current_stage: Some(GameFutureStage::ReadyForTurn),
        }
    }

    fn poll_ready(&mut self) -> bool {
        let turn = self.game.as_ref().unwrap().state.turn.clone();
        println!("poll_ready {:?}", turn.clone());
        let new_turn_future = self.players.take().unwrap().new_turn(turn);
        self.current_stage = Some(GameFutureStage::StartTurn(new_turn_future));
        return false;
    }

    fn poll_start_turn(&mut self, future: &mut BoxFutureNotSend<Clients<S, T>, ()>) -> bool {
        println!("poll_start_turn");
        self.players = match future.poll() {
            Ok(Async::Ready(players)) => Some(players),
            _ => return true,
        };

        // @TODO: Have ask_moves take keys() directly.
        let living_player_names =
            self.game.as_ref().unwrap().state.turn.snakes.keys().cloned().collect();
        let ask_moves_future = self.players.take().unwrap().ask_moves(&living_player_names);
        self.current_stage = Some(GameFutureStage::AskMoves(ask_moves_future));
        return false;
    }

    fn poll_ask_moves(&mut self,
                      future: &mut BoxFutureNotSend<(HashMap<String, MoveMsg>, Clients<S, T>),
                                                    ()>) -> bool {
        println!("poll_ask_moves");
        let (move_msgs, players) = match future.poll() {
            Ok(Async::Ready((move_msgs, players))) => (move_msgs, players),
            _ => return true,
        };
        self.players = Some(players);

        self.current_stage = Some(GameFutureStage::AdvanceTurn(move_msgs));
        return false;
    }

    fn poll_advance_turn(&mut self, move_msgs: &mut HashMap<String, MoveMsg>) -> bool {
        println!("poll_advance_turn");
        // @TODO: Have advance_turn take MoveMsgs.
        let moves = move_msgs.into_iter()
            .map(|(name, move_msg)| (name.clone(), move_msg.direction));
        self.game.as_mut().unwrap().advance_turn(moves.collect());

        let ref casualties = self.game.as_ref().unwrap().state.turn.casualties;
        let die_future = self.players.take().unwrap().die(&casualties);
        self.current_stage = Some(GameFutureStage::NotifyDead(die_future));
        return false;
    }

    fn poll_notify_dead(&mut self, future: &mut BoxFutureNotSend<Clients<S, T>, ()>) -> bool {
        println!("poll_notify_dead");
        self.players = match future.poll() {
            Ok(Async::Ready(players)) => Some(players),
            _ => return true,
        };

        self.current_stage = Some(GameFutureStage::LoopDecision);
        return false;
    }

    fn poll_loop_decision(&mut self) -> bool {
        println!("poll_loop_decision");
        if self.game.as_ref().unwrap().concluded() {
            self.current_stage = Some(GameFutureStage::Concluded);
        } else {
            self.current_stage = Some(GameFutureStage::ReadyForTurn);
        }
        return false;
    }
}

impl<S, T, R> Future for GameFuture<S, T, R>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send + 'static,
          T: Stream<Item = Msg, Error = io::Error> + Send + 'static,
          R: Rng
{
    type Item = (Engine<R>, Clients<S, T>);
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            let stop_poll = match self.current_stage.take().unwrap() {
                GameFutureStage::ReadyForTurn => self.poll_ready(),
                GameFutureStage::StartTurn(ref mut future) => self.poll_start_turn(future),
                GameFutureStage::AskMoves(ref mut future) => self.poll_ask_moves(future),
                GameFutureStage::AdvanceTurn(ref mut move_msgs) => self.poll_advance_turn(move_msgs),
                GameFutureStage::NotifyDead(ref mut future) => self.poll_notify_dead(future),
                GameFutureStage::LoopDecision => self.poll_loop_decision(),
                GameFutureStage::Concluded => {
                    let return_pair = (self.game.take().unwrap(), self.players.take().unwrap());
                    return Ok(Async::Ready(return_pair))
                }
            };
            if stop_poll == true {
                // @TODO: Verify this is how to suspend for a little bit to keep the rest of the
                // event loop going.
                // self.remote.spawn(|handle| self);
                return Ok(Async::NotReady);
            }
        }

        // Syncronous blocking version.
        // loop {
        // let players = self.players.new_turn(self.turn.clone()).wait();
        // let (moves, players) = self.players.ask_moves(self.turn.snakes.keys()).wait();
        // self.advance_turn(moves);
        // let players = self.players.die(&self.turn.casualties).wait();
        // if self.concluded() {
        // return Ok(Async::Ready(*self));
        // }
        // }
    }
}

enum GameFutureStage<S, T>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send + 'static,
          T: Stream<Item = Msg, Error = io::Error> + Send + 'static
{
    ReadyForTurn,
    StartTurn(BoxFutureNotSend<Clients<S, T>, ()>),
    AskMoves(BoxFutureNotSend<(HashMap<String, MoveMsg>, Clients<S, T>), ()>),
    AdvanceTurn(HashMap<String, MoveMsg>),
    NotifyDead(BoxFutureNotSend<Clients<S, T>, ()>),
    LoopDecision,
    Concluded,
}
