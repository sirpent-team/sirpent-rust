use std::io;
use std::marker::Send;
use std::collections::HashMap;
use rand::Rng;

use futures::{future, Async, Future, Stream, Sink, Poll};

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

enum GameFutureStage<S, T>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send + 'static,
          T: Stream<Item = Msg, Error = io::Error> + Send + 'static
{
    StartOfGame,
    ReadyForTurn(BoxFutureNotSend<Clients<S, T>, ()>),
    StartTurn(BoxFutureNotSend<Clients<S, T>, ()>),
    AskMoves(BoxFutureNotSend<(HashMap<String, MoveMsg>, Clients<S, T>), ()>),
    AdvanceTurn(HashMap<String, MoveMsg>),
    NotifyDead(BoxFutureNotSend<Clients<S, T>, ()>),
    LoopDecision,
    Concluded,
}

enum GameFutureStageControl {
    Continue,
    Suspend,
}

use self::GameFutureStage::*;
use self::GameFutureStageControl::*;

type GameFuturePollReturn<S, T> = (GameFutureStage<S, T>, GameFutureStageControl);

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
            current_stage: Some(StartOfGame),
        }
    }

    fn poll_start_of_game(&mut self) -> GameFuturePollReturn<S, T> {
        let game = self.game.as_ref().unwrap().state.game.clone();
        let new_game_future = self.players
            .take()
            .unwrap()
            .new_game(game);
        return (ReadyForTurn(new_game_future), Continue);
    }

    fn poll_ready_for_turn(&mut self,
                           mut future: BoxFutureNotSend<Clients<S, T>, ()>)
                           -> GameFuturePollReturn<S, T> {
        self.players = match future.poll() {
            Ok(Async::Ready(players)) => Some(players),
            _ => return (GameFutureStage::ReadyForTurn(future), Suspend),
        };

        let turn = self.game.as_ref().unwrap().state.turn.clone();
        let new_turn_future = self.players
            .take()
            .unwrap()
            .new_turn(turn);
        return (StartTurn(new_turn_future), Continue);
    }

    fn poll_start_turn(&mut self,
                       mut future: BoxFutureNotSend<Clients<S, T>, ()>)
                       -> GameFuturePollReturn<S, T> {
        self.players = match future.poll() {
            Ok(Async::Ready(players)) => Some(players),
            _ => return (GameFutureStage::StartTurn(future), Suspend),
        };

        // @TODO: Have ask_moves take keys() directly.
        let living_player_names =
            self.game.as_ref().unwrap().state.turn.snakes.keys().cloned().collect();
        let ask_moves_future = self.players.take().unwrap().ask_moves(&living_player_names);
        return (GameFutureStage::AskMoves(ask_moves_future), Continue);
    }

    fn poll_ask_moves(&mut self,
                      mut future: BoxFutureNotSend<(HashMap<String, MoveMsg>, Clients<S, T>), ()>)
                      -> GameFuturePollReturn<S, T> {
        let (move_msgs, players) = match future.poll() {
            Ok(Async::Ready((move_msgs, players))) => (move_msgs, players),
            _ => return (GameFutureStage::AskMoves(future), Suspend),
        };
        self.players = Some(players);

        return (GameFutureStage::AdvanceTurn(move_msgs), Continue);
    }

    fn poll_advance_turn(&mut self,
                         move_msgs: HashMap<String, MoveMsg>)
                         -> GameFuturePollReturn<S, T> {
        // @TODO: Have advance_turn take MoveMsgs.
        let moves = move_msgs.into_iter()
            .map(|(name, move_msg)| (name.clone(), move_msg.direction));
        self.game.as_mut().unwrap().advance_turn(moves.collect());

        let ref new_turn = self.game.as_ref().unwrap().state.turn;
        println!("Advanced turn to {:?}", new_turn.clone());

        let die_future = self.players.take().unwrap().die(&new_turn.casualties);
        return (GameFutureStage::NotifyDead(die_future), Continue);
    }

    fn poll_notify_dead(&mut self,
                        mut future: BoxFutureNotSend<Clients<S, T>, ()>)
                        -> GameFuturePollReturn<S, T> {
        self.players = match future.poll() {
            Ok(Async::Ready(players)) => Some(players),
            _ => return (GameFutureStage::NotifyDead(future), Suspend),
        };

        return (GameFutureStage::LoopDecision, Continue);
    }

    fn poll_loop_decision(&mut self) -> GameFuturePollReturn<S, T> {
        if self.game.as_ref().unwrap().concluded() {
            return (GameFutureStage::Concluded, Continue);
        } else {
            // Returns players despite no future being run. Believed negligible-cost.
            let players_done = Box::new(future::ok(self.players.take().unwrap()));
            return (GameFutureStage::ReadyForTurn(players_done), Continue);
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
        loop {
            assert!(self.current_stage.is_some());

            let (new_stage, stage_control) = match self.current_stage.take().unwrap() {
                GameFutureStage::StartOfGame => self.poll_start_of_game(),
                GameFutureStage::ReadyForTurn(future) => self.poll_ready_for_turn(future),
                GameFutureStage::StartTurn(future) => self.poll_start_turn(future),
                GameFutureStage::AskMoves(future) => self.poll_ask_moves(future),
                GameFutureStage::AdvanceTurn(move_msgs) => self.poll_advance_turn(move_msgs),
                GameFutureStage::NotifyDead(future) => self.poll_notify_dead(future),
                GameFutureStage::LoopDecision => self.poll_loop_decision(),
                GameFutureStage::Concluded => {
                    let return_pair = (self.game.take().unwrap(), self.players.take().unwrap());
                    return Ok(Async::Ready(return_pair));
                }
            };
            self.current_stage = Some(new_stage);
            match stage_control {
                Continue => continue,
                Suspend => return Ok(Async::NotReady),
            }
        }
    }
}
