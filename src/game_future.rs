use std::io;
use std::marker::Send;
use std::collections::HashMap;
use rand::Rng;

use futures::{future, Async, Future, Stream, Sink, Poll};

use game::*;
use clients::*;
use protocol::*;

pub struct GameFuture<S, T, R>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send + 'static,
          T: Stream<Item = Msg, Error = io::Error> + Send + 'static,
          R: Rng
{
    game: Option<Game<R>>,
    players: Option<Clients<S, T>>,
    spectators: Option<Clients<S, T>>,
    current_stage: Option<GameFutureStage<S, T>>,
}

enum GameFutureStage<S, T>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send + 'static,
          T: Stream<Item = Msg, Error = io::Error> + Send + 'static
{
    StartOfGame,
    ReadyForTurn(BoxedFuture<(Clients<S, T>, Clients<S, T>), ()>),
    StartTurn(BoxedFuture<(Clients<S, T>, Clients<S, T>), ()>),
    AskMoves(BoxedFuture<(HashMap<String, ProtocolResult<MoveMsg>>, Clients<S, T>), ()>),
    AdvanceTurn(HashMap<String, ProtocolResult<MoveMsg>>),
    NotifyDead(BoxedFuture<Clients<S, T>, ()>),
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
    pub fn new(mut game: Game<R>, players: Clients<S, T>, spectators: Clients<S, T>) -> Self {
        for name in players.names() {
            game.add_player(name.clone());
        }

        GameFuture {
            game: Some(game),
            players: Some(players),
            spectators: Some(spectators),
            current_stage: Some(StartOfGame),
        }
    }

    fn start_of_game(&mut self) -> GameFuturePollReturn<S, T> {
        let game = self.game.as_ref().unwrap().game_state.clone();
        let new_game_future = self.players
            .take()
            .unwrap()
            .new_game(game.clone());
        let spectators = self.spectators.take().unwrap();
        let new_game_future = box new_game_future.and_then(move |players| {
            spectators.new_game(game).map(|spectators| (players, spectators))
        });
        return (ReadyForTurn(new_game_future), Continue);
    }

    fn ready_for_turn(&mut self,
                      mut future: BoxedFuture<(Clients<S, T>, Clients<S, T>), ()>)
                      -> GameFuturePollReturn<S, T> {
        let (players, spectators) = match future.poll() {
            Ok(Async::Ready(p)) => p,
            _ => return (GameFutureStage::ReadyForTurn(future), Suspend),
        };
        self.players = Some(players);
        self.spectators = Some(spectators);

        let turn = self.game.as_ref().unwrap().turn_state.clone();
        let new_turn_future = self.players
            .take()
            .unwrap()
            .new_turn(turn.clone());
        let spectators = self.spectators.take().unwrap();
        let new_turn_future =
            box new_turn_future.and_then(|players| {
                spectators.new_turn(turn).map(|spectators| (players, spectators))
            });
        return (StartTurn(new_turn_future), Continue);
    }

    fn start_turn(&mut self,
                  mut future: BoxedFuture<(Clients<S, T>, Clients<S, T>), ()>)
                  -> GameFuturePollReturn<S, T> {
        let (players, spectators) = match future.poll() {
            Ok(Async::Ready(p)) => p,
            _ => return (GameFutureStage::StartTurn(future), Suspend),
        };
        self.players = Some(players);
        self.spectators = Some(spectators);

        // @TODO: Have ask_moves take keys() directly.
        let living_player_names =
            self.game.as_ref().unwrap().turn_state.snakes.keys().cloned().collect();
        let ask_moves_future = self.players.take().unwrap().ask_moves(&living_player_names);
        return (GameFutureStage::AskMoves(ask_moves_future), Continue);
    }

    fn ask_moves(&mut self,
                 mut future: BoxedFuture<(HashMap<String, ProtocolResult<MoveMsg>>,
                                          Clients<S, T>),
                                         ()>)
                 -> GameFuturePollReturn<S, T> {
        let (move_msgs, players) = match future.poll() {
            Ok(Async::Ready((move_msgs, players))) => (move_msgs, players),
            _ => return (GameFutureStage::AskMoves(future), Suspend),
        };
        self.players = Some(players);

        return (GameFutureStage::AdvanceTurn(move_msgs), Continue);
    }

    fn advance_turn(&mut self,
                    move_msgs: HashMap<String, ProtocolResult<MoveMsg>>)
                    -> GameFuturePollReturn<S, T> {
        // @TODO: Have advance_turn take MoveMsgs.
        let moves = move_msgs.into_iter()
            .map(|(name, move_msg)| {
                (name.clone(), move_msg.and_then(|move_msg| Ok(move_msg.direction)))
            });
        self.game.as_mut().unwrap().advance_turn(moves.collect());

        let ref new_turn = self.game.as_ref().unwrap().turn_state;
        println!("Advanced turn to {:?}", new_turn.clone());

        let die_future = self.players.take().unwrap().die(&new_turn.casualties);
        return (GameFutureStage::NotifyDead(die_future), Continue);
    }

    fn notify_dead(&mut self,
                   mut future: BoxedFuture<Clients<S, T>, ()>)
                   -> GameFuturePollReturn<S, T> {
        self.players = match future.poll() {
            Ok(Async::Ready(players)) => Some(players),
            _ => return (GameFutureStage::NotifyDead(future), Suspend),
        };

        return (GameFutureStage::LoopDecision, Continue);
    }

    fn loop_decision(&mut self) -> GameFuturePollReturn<S, T> {
        if self.game.as_ref().unwrap().concluded() {
            return (GameFutureStage::Concluded, Continue);
        } else {
            // Returns players despite no future being run. Believed negligible-cost.
            let players = self.players.take().unwrap();
            let spectators = self.spectators.take().unwrap();
            let players_done = box future::ok((players, spectators));
            return (GameFutureStage::ReadyForTurn(players_done), Continue);
        }
    }
}

impl<S, T, R> Future for GameFuture<S, T, R>
    where S: Sink<SinkItem = Msg, SinkError = io::Error> + Send + 'static,
          T: Stream<Item = Msg, Error = io::Error> + Send + 'static,
          R: Rng
{
    type Item = (Game<R>, Clients<S, T>, Clients<S, T>);
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            assert!(self.current_stage.is_some());

            let (new_stage, stage_control) = match self.current_stage.take().unwrap() {
                GameFutureStage::StartOfGame => self.start_of_game(),
                GameFutureStage::ReadyForTurn(future) => self.ready_for_turn(future),
                GameFutureStage::StartTurn(future) => self.start_turn(future),
                GameFutureStage::AskMoves(future) => self.ask_moves(future),
                GameFutureStage::AdvanceTurn(move_msgs) => self.advance_turn(move_msgs),
                GameFutureStage::NotifyDead(future) => self.notify_dead(future),
                GameFutureStage::LoopDecision => self.loop_decision(),
                GameFutureStage::Concluded => {
                    let game = self.game.take().unwrap();
                    let players = self.players.take().unwrap();
                    let spectators = self.spectators.take().unwrap();
                    let return_triple = (game, players, spectators);
                    return Ok(Async::Ready(return_triple));
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
