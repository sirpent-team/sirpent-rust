use std::io;
use rand::Rng;
use std::marker::Send;
use std::time::Duration;
use std::collections::HashMap;
use futures::{future, Async, BoxFuture, Future, Sink, Poll};

use game::*;
use net::*;
use clients::*;

fn retain_oks<O, E>(h: HashMap<String, Result<O, E>>) -> HashMap<String, O> {
    h.into_iter()
        .filter_map(|(id, result)| {
            match result {
                Ok(o) => Some((id, o)),
                Err(_) => None,
            }
        })
        .collect()
}

pub struct GameFuture<CmdSink, R>
    where CmdSink: Sink<SinkItem = Cmd> + Send + 'static,
          CmdSink::SinkError: Send + 'static,
          R: Rng
{
    game: Option<Game<R>>,
    players: Option<HashMap<String, CmdSink>>,
    spectators: Option<HashMap<String, CmdSink>>,
    current_stage: Option<GameFutureStage<CmdSink>>,
    timeout: Option<Duration>,
}

enum GameFutureStage<CmdSink>
    where CmdSink: Sink<SinkItem = Cmd> + Send + 'static,
          CmdSink::SinkError: Send + 'static
{
    StartOfGame,
    ReadyForTurn(BoxFuture<(HashMap<String, CmdSink>, HashMap<String, CmdSink>), io::Error>),
    StartTurn(BoxFuture<(HashMap<String, CmdSink>, HashMap<String, CmdSink>), io::Error>),
    AskMoves(BoxFuture<(HashMap<String, (Msg, CmdSink)>), io::Error>),
    AdvanceTurn(HashMap<String, Msg>),
    NotifyDead(BoxFuture<HashMap<String, CmdSink>, io::Error>),
    LoopDecision,
    Concluded,
}

enum GameFutureStageControl {
    Continue,
    Suspend,
}

use self::GameFutureStage::*;
use self::GameFutureStageControl::*;

type GameFuturePollReturn<CmdSink> = (GameFutureStage<CmdSink>, GameFutureStageControl);

impl<CmdSink, R> GameFuture<CmdSink, R>
    where CmdSink: Sink<SinkItem = Cmd> + Send + 'static,
          CmdSink::SinkError: Send + 'static,
          R: Rng
{
    pub fn new(mut game: Game<R>,
               players: HashMap<String, CmdSink>,
               spectators: HashMap<String, CmdSink>,
               timeout: Option<Duration>)
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

    fn start_of_game(&mut self) -> GameFuturePollReturn<CmdSink> {
        let game = self.game.as_ref().unwrap().game_state.clone();
        let new_game_msg = Msg::NewGame { game: game };

        let players = self.players.take().unwrap();
        let spectators = self.spectators.take().unwrap();

        let f1 = group_transmit(players, MessageMode::Constant(new_game_msg.clone()))
            .map(retain_oks);
        let f2 = group_transmit(spectators, MessageMode::Constant(new_game_msg)).map(retain_oks);
        let new_game_future = f1.join(f2).boxed();
        return (ReadyForTurn(new_game_future), Continue);
    }

    fn ready_for_turn(&mut self,
                      mut future: BoxFuture<(HashMap<String, CmdSink>, HashMap<String, CmdSink>),
                                            io::Error>)
                      -> GameFuturePollReturn<CmdSink> {
        let (players, spectators) = match future.poll() {
            Ok(Async::Ready(pair)) => pair,
            _ => return (GameFutureStage::ReadyForTurn(future), Suspend),
        };

        let turn = self.game.as_ref().unwrap().turn_state.clone();
        let turn_msg = Msg::Turn { turn: turn };

        let f1 = group_transmit(players, MessageMode::Constant(turn_msg.clone())).map(retain_oks);
        let f2 = group_transmit(spectators, MessageMode::Constant(turn_msg)).map(retain_oks);
        let turn_future = f1.join(f2).boxed();
        return (StartTurn(turn_future), Continue);
    }

    fn start_turn(&mut self,
                  mut future: BoxFuture<(HashMap<String, CmdSink>, HashMap<String, CmdSink>),
                                        io::Error>)
                  -> GameFuturePollReturn<CmdSink> {
        let (mut players, spectators) = match future.poll() {
            Ok(Async::Ready(pair)) => pair,
            _ => return (GameFutureStage::StartTurn(future), Suspend),
        };
        self.spectators = Some(spectators);

        let turn = self.game.as_ref().unwrap().turn_state.clone();
        let (living_players, dead_players) = players.drain()
            .partition(|&(ref name, _)| turn.snakes.contains_key(name));
        self.players = Some(dead_players);

        let move_future = group_receive(living_players, self.timeout).map(retain_oks).boxed();
        return (GameFutureStage::AskMoves(move_future), Continue);
    }

    fn ask_moves(&mut self,
                 mut future: BoxFuture<(HashMap<String, (Msg, CmdSink)>), io::Error>)
                 -> GameFuturePollReturn<CmdSink> {
        let mut answers = match future.poll() {
            Ok(Async::Ready(answers)) => answers,
            _ => return (GameFutureStage::AskMoves(future), Suspend),
        };
        let mut living_players = HashMap::with_capacity(answers.len());
        let msgs = answers.drain()
            .map(|(name, (msg, cmd_tx))| {
                living_players.insert(name.clone(), cmd_tx);
                (name, msg)
            })
            .collect();

        self.players.as_mut().unwrap().extend(living_players.into_iter());

        return (GameFutureStage::AdvanceTurn(msgs), Continue);
    }

    fn advance_turn(&mut self, mut moves: HashMap<String, Msg>) -> GameFuturePollReturn<CmdSink> {
        let directions = moves.drain().filter_map(|(name, msg)| {
            if let Msg::Move { direction } = msg {
                Some((name.clone(), Ok(direction)))
            } else {
                None
            }
        });
        self.game.as_mut().unwrap().advance_turn(directions.collect());

        let ref new_turn = self.game.as_ref().unwrap().turn_state;
        println!("Advanced turn to {:?}", new_turn.clone());

        let mut players = self.players
            .take()
            .unwrap();
        let (casualty_players, living_players) = players.drain()
            .partition(|&(ref name, _)| new_turn.casualties.contains_key(name));
        self.players = Some(living_players);
        let die_future = group_transmit(casualty_players, MessageMode::Constant(Msg::Died))
            .map(retain_oks)
            .boxed();
        return (GameFutureStage::NotifyDead(die_future), Continue);
    }

    fn notify_dead(&mut self,
                   mut future: BoxFuture<HashMap<String, CmdSink>, io::Error>)
                   -> GameFuturePollReturn<CmdSink> {
        let casualty_players = match future.poll() {
            Ok(Async::Ready(players)) => players,
            _ => return (GameFutureStage::NotifyDead(future), Suspend),
        };
        self.players.as_mut().unwrap().extend(casualty_players.into_iter());

        return (GameFutureStage::LoopDecision, Continue);
    }

    fn loop_decision(&mut self) -> GameFuturePollReturn<CmdSink> {
        if self.game.as_ref().unwrap().concluded() {
            return (GameFutureStage::Concluded, Continue);
        } else {
            // Returns players despite no future being run. Believed negligible-cost.
            let players = self.players.take().unwrap();
            let spectators = self.spectators.take().unwrap();
            let players_done = future::ok((players, spectators)).boxed();
            return (GameFutureStage::ReadyForTurn(players_done), Continue);
        }
    }
}

impl<CmdSink, R> Future for GameFuture<CmdSink, R>
    where CmdSink: Sink<SinkItem = Cmd> + Send + 'static,
          CmdSink::SinkError: Send + 'static,
          R: Rng
{
    type Item = (Game<R>, HashMap<String, CmdSink>, HashMap<String, CmdSink>);
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
