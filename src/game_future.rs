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
    current_stage: Option<GameFutureStage<S, T>>,
    players: Option<Clients<S, T>>,
    game: Option<Engine<R>>, /* turn_state: TurnState,
                              * game_state: GameState */
}

impl<S, T, R> GameFuture<S, T, R>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send + 'static,
          T: Stream<Item = Msg, Error = io::Error> + Send + 'static,
          R: Rng
{
    fn poll_ready(&mut self) {
        let turn = self.game.as_ref().unwrap().state.turn.clone();
        let new_turn_future = self.players.take().unwrap().new_turn(turn);
        self.current_stage = Some(GameFutureStage::StartTurn(new_turn_future));
    }

    fn poll_start_turn(&mut self, future: &mut BoxFutureNotSend<Clients<S, T>, ()>) {
        self.players = match future.poll() {
            Ok(Async::Ready(players)) => Some(players),
            _ => return,
        };

        // @TODO: Have ask_moves take keys() directly.
        let living_player_names =
            self.game.as_ref().unwrap().state.turn.snakes.keys().cloned().collect();
        let ask_moves_future = self.players.take().unwrap().ask_moves(&living_player_names);
        self.current_stage = Some(GameFutureStage::AskMoves(ask_moves_future));
    }

    fn poll_ask_moves(&mut self,
                      future: &mut BoxFutureNotSend<(HashMap<String, MoveMsg>, Clients<S, T>),
                                                    ()>) {
        let (move_msgs, players) = match future.poll() {
            Ok(Async::Ready((move_msgs, players))) => (move_msgs, players),
            _ => return,
        };
        self.players = Some(players);

        self.current_stage = Some(GameFutureStage::AdvanceTurn(move_msgs));
    }

    fn poll_advance_turn(&mut self, move_msgs: &mut HashMap<String, MoveMsg>) {
        // @TODO: Have advance_turn take MoveMsgs.
        let moves = move_msgs.into_iter()
            .map(|(name, move_msg)| (name.clone(), move_msg.direction));
        self.game.as_mut().unwrap().advance_turn(moves.collect());

        let ref casualties = self.game.as_ref().unwrap().state.turn.casualties;
        let die_future = self.players.take().unwrap().die(&casualties);
        self.current_stage = Some(GameFutureStage::NotifyDead(die_future));
    }

    fn poll_notify_dead(&mut self, future: &mut BoxFutureNotSend<Clients<S, T>, ()>) {
        self.players = match future.poll() {
            Ok(Async::Ready(players)) => Some(players),
            _ => return,
        };

        self.current_stage = Some(GameFutureStage::LoopDecision);
    }

    fn poll_loop_decision(&mut self) {
        if self.game.as_ref().unwrap().concluded() {
            self.current_stage = Some(GameFutureStage::Concluded);
        } else {
            self.current_stage = Some(GameFutureStage::ReadyForTurn);
        }
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
        match self.current_stage.take().unwrap() {
            GameFutureStage::ReadyForTurn => self.poll_ready(),
            GameFutureStage::StartTurn(ref mut future) => self.poll_start_turn(future),
            GameFutureStage::AskMoves(ref mut future) => self.poll_ask_moves(future),
            GameFutureStage::AdvanceTurn(ref mut move_msgs) => self.poll_advance_turn(move_msgs),
            GameFutureStage::NotifyDead(ref mut future) => self.poll_notify_dead(future),
            GameFutureStage::LoopDecision => self.poll_loop_decision(),
            GameFutureStage::Concluded => {
                return Ok(Async::Ready((self.game.take().unwrap(), self.players.take().unwrap())))
            }
        };
        // @TODO: Verify this is how to suspend for a little bit to keep the rest of the event
        // loop going.
        // self.remote.spawn(|handle| self);
        return Ok(Async::NotReady);

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
