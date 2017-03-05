use rand::Rng;
use std::marker::Send;
use std::collections::HashMap;
use futures::{future, Async, BoxFuture, Future, Sink, Poll};

use super::*;
use net::*;
use net::clients::*;
use utils::*;

pub struct GameFuture<CmdSink, R>
    where CmdSink: Sink<SinkItem = Cmd, SinkError = Error> + Send + 'static,
          R: Rng
{
    game: Option<Game<R>>,
    players: Option<HashMap<String, CmdSink>>,
    spectators: Option<HashMap<String, CmdSink>>,
    current_stage: Option<GameFutureStage<CmdSink>>,
    timeout: Option<Milliseconds>,
}

enum GameFutureStage<CmdSink>
    where CmdSink: Sink<SinkItem = Cmd, SinkError = Error> + Send + 'static
{
    StartOfGame,
    ReadyForRound(BoxFuture<(HashMap<String, CmdSink>, HashMap<String, CmdSink>), Error>),
    StartRound(BoxFuture<(HashMap<String, CmdSink>, HashMap<String, CmdSink>), Error>),
    AskMoves(BoxFuture<(HashMap<String, (Msg, CmdSink)>), Error>),
    AdvanceRound(HashMap<String, Msg>),
    LoopDecision,
    Concluding(BoxFuture<(HashMap<String, CmdSink>, HashMap<String, CmdSink>), Error>),
    EndOfGame,
}

enum GameFutureStageControl {
    Continue,
    Suspend,
}

use self::GameFutureStage::*;
use self::GameFutureStageControl::*;

type GameFuturePollreturn<CmdSink> = (GameFutureStage<CmdSink>, GameFutureStageControl);

impl<CmdSink, R> GameFuture<CmdSink, R>
    where CmdSink: Sink<SinkItem = Cmd, SinkError = Error> + Send + 'static,
          R: Rng
{
    pub fn new(mut game: Game<R>,
               players: HashMap<String, CmdSink>,
               spectators: HashMap<String, CmdSink>,
               timeout: Option<Milliseconds>)
               -> Self {
        for name in players.keys() {
            game.add_player(name.clone());
        }

        GameFuture {
            game: Some(game),
            players: Some(players),
            spectators: Some(spectators),
            current_stage: Some(StartOfGame),
            timeout: timeout,
        }
    }

    fn start_of_game(&mut self) -> GameFuturePollreturn<CmdSink> {
        let game = self.game.as_ref().unwrap().game_state.clone();
        let new_game_msg = Msg::Game { game: game };

        let players = self.players.take().unwrap();
        let spectators = self.spectators.take().unwrap();

        let f1 = group_transmit(players, MessageMode::Constant(new_game_msg.clone()))
            .map(retain_oks);
        let f2 = group_transmit(spectators, MessageMode::Constant(new_game_msg)).map(retain_oks);
        let new_game_future = f1.join(f2).boxed();
        (ReadyForRound(new_game_future), Continue)
    }

    fn ready_for_round(&mut self,
                       mut future: BoxFuture<(HashMap<String, CmdSink>,
                                              HashMap<String, CmdSink>),
                                             Error>)
                       -> GameFuturePollreturn<CmdSink> {
        let (players, spectators) = match future.poll() {
            Ok(Async::Ready(pair)) => pair,
            _ => return (ReadyForRound(future), Suspend),
        };

        let game = self.game.as_ref().unwrap();
        let round = game.round_state.clone();
        let game_uuid = game.game_state.uuid;
        let round_msg = Msg::Round {
            round: round,
            game_uuid: game_uuid,
        };

        let players_txing = group_transmit(players, MessageMode::Constant(round_msg.clone()))
            .map(retain_oks);
        let spectators_txing = group_transmit(spectators, MessageMode::Constant(round_msg))
            .map(retain_oks);
        let round_future = players_txing.join(spectators_txing).boxed();
        (StartRound(round_future), Continue)
    }

    fn start_round(&mut self,
                   mut future: BoxFuture<(HashMap<String, CmdSink>, HashMap<String, CmdSink>),
                                         Error>)
                   -> GameFuturePollreturn<CmdSink> {
        let (mut players, spectators) = match future.poll() {
            Ok(Async::Ready(pair)) => pair,
            _ => return (StartRound(future), Suspend),
        };
        self.spectators = Some(spectators);

        let round = self.game.as_ref().unwrap().round_state.clone();
        let (living_players, dead_players) = players.drain()
            .partition(|&(ref name, _)| round.snakes.contains_key(name));
        self.players = Some(dead_players);

        let move_future = group_receive(living_players, self.timeout).map(retain_oks).boxed();
        (AskMoves(move_future), Continue)
    }

    fn ask_moves(&mut self,
                 mut future: BoxFuture<(HashMap<String, (Msg, CmdSink)>), Error>)
                 -> GameFuturePollreturn<CmdSink> {
        let mut answers = match future.poll() {
            Ok(Async::Ready(answers)) => answers,
            _ => return (AskMoves(future), Suspend),
        };
        let mut living_players = HashMap::with_capacity(answers.len());
        let msgs = answers.drain()
            .map(|(name, (msg, cmd_tx))| {
                living_players.insert(name.clone(), cmd_tx);
                (name, msg)
            })
            .collect();

        self.players.as_mut().unwrap().extend(living_players.into_iter());

        (AdvanceRound(msgs), Continue)
    }

    fn advance_round(&mut self, mut moves: HashMap<String, Msg>) -> GameFuturePollreturn<CmdSink> {
        let directions = moves.drain()
            .filter_map(|(name, msg)| if let Msg::Move { direction } = msg {
                Some((name.clone(), Ok(direction)))
            } else {
                None
            });
        self.game.as_mut().unwrap().advance_round(directions.collect());

        let new_round = &self.game.as_ref().unwrap().round_state;
        println!("Advanced round to {:?}", new_round.clone());

        (LoopDecision, Continue)
    }

    fn loop_decision(&mut self) -> GameFuturePollreturn<CmdSink> {
        if self.game.as_ref().unwrap().concluded() {
            let game = self.game.as_ref().unwrap();
            let round = game.round_state.clone();
            let game_uuid = game.game_state.uuid;
            let game_over_msg = Msg::outcome(round, game_uuid);

            let players = self.players.take().unwrap();
            let spectators = self.spectators.take().unwrap();

            let players_txing = group_transmit(players,
                                               MessageMode::Constant(game_over_msg.clone()))
                .map(retain_oks);
            let spectators_txing = group_transmit(spectators, MessageMode::Constant(game_over_msg))
                .map(retain_oks);
            let concluding_future = players_txing.join(spectators_txing).boxed();
            (Concluding(concluding_future), Continue)
        } else {
            // returns players despite no future being run. Believed negligible-cost.
            let players = self.players.take().unwrap();
            let spectators = self.spectators.take().unwrap();
            let players_done = future::ok((players, spectators)).boxed();
            (ReadyForRound(players_done), Continue)
        }
    }

    fn conclude(&mut self,
                mut future: BoxFuture<(HashMap<String, CmdSink>, HashMap<String, CmdSink>),
                                      Error>)
                -> GameFuturePollreturn<CmdSink> {
        let (players, spectators) = match future.poll() {
            Ok(Async::Ready(pair)) => pair,
            _ => return (StartRound(future), Suspend),
        };

        self.players = Some(players);
        self.spectators = Some(spectators);
        (EndOfGame, Continue)
    }
}

impl<CmdSink, R> Future for GameFuture<CmdSink, R>
    where CmdSink: Sink<SinkItem = Cmd, SinkError = Error> + Send + 'static,
          R: Rng
{
    type Item = (Game<R>, HashMap<String, CmdSink>, HashMap<String, CmdSink>);
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            assert!(self.current_stage.is_some());

            let (new_stage, stage_control) = match self.current_stage.take().unwrap() {
                StartOfGame => self.start_of_game(),
                ReadyForRound(future) => self.ready_for_round(future),
                StartRound(future) => self.start_round(future),
                AskMoves(future) => self.ask_moves(future),
                AdvanceRound(move_msgs) => self.advance_round(move_msgs),
                LoopDecision => self.loop_decision(),
                Concluding(future) => self.conclude(future),
                EndOfGame => {
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
