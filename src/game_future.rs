use std::marker::Send;
use std::collections::HashMap;
use rand::Rng;
use std::time::Duration;

use futures::{future, Async, Future, Sink, Poll};
use tokio_timer::Timer;

use game::*;
use protocol::*;
use client_future::*;

pub type BoxedFuture<I, E> = Box<Future<Item = I, Error = E>>;

pub struct GameFuture<C, R>
    where C: Sink<SinkItem = ClientFutureCommand<String>, SinkError = ()> + Send + 'static,
          R: Rng
{
    game: Option<Game<R>>,
    players: Option<HashMap<String, C>>,
    spectators: Option<HashMap<String, C>>,
    current_stage: Option<GameFutureStage<C>>,
    timeout: Duration,
    timer: Timer,
}

enum GameFutureStage<C>
    where C: Sink<SinkItem = ClientFutureCommand<String>, SinkError = ()> + Send + 'static
{
    StartOfGame,
    ReadyForTurn(BoxedFuture<(HashMap<String, C>, HashMap<String, C>), ()>),
    StartTurn(BoxedFuture<(HashMap<String, C>, HashMap<String, C>), ()>),
    AskMoves(BoxedFuture<(HashMap<String, Msg>, HashMap<String, C>), ()>),
    AdvanceTurn(HashMap<String, Msg>),
    NotifyDead(BoxedFuture<HashMap<String, C>, ()>),
    LoopDecision,
    Concluded,
}

enum GameFutureStageControl {
    Continue,
    Suspend,
}

use self::GameFutureStage::*;
use self::GameFutureStageControl::*;

type GameFuturePollReturn<C> = (GameFutureStage<C>, GameFutureStageControl);

impl<C, R> GameFuture<C, R>
    where C: Sink<SinkItem = ClientFutureCommand<String>, SinkError = ()> + Send + 'static,
          R: Rng
{
    pub fn new(mut game: Game<R>,
               players: HashMap<String, C>,
               spectators: HashMap<String, C>,
               timeout: Duration,
               timer: Timer)
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
            timer: timer,
        }
    }

    fn start_of_game(&mut self) -> GameFuturePollReturn<C> {
        let game = self.game.as_ref().unwrap().game_state.clone();
        let new_game_msg = Msg::NewGame { game: game };

        let players = self.players.take().unwrap();
        let spectators = self.spectators.take().unwrap();

        let new_game_future =
            ClientsSend::<String, C, BoxedFuture<C, ()>>::new(players, new_game_msg.clone())
                .and_then(move |players| {
                    ClientsSend::<String, C, BoxedFuture<C, ()>>::new(spectators, new_game_msg)
                        .map(|spectators| (players, spectators))
                })
                .boxed();
        return (ReadyForTurn(new_game_future), Continue);
    }

    fn ready_for_turn(&mut self,
                      mut future: BoxedFuture<(HashMap<String, C>, HashMap<String, C>), ()>)
                      -> GameFuturePollReturn<C> {
        let (players, spectators) = match future.poll() {
            Ok(Async::Ready(pair)) => pair,
            _ => return (GameFutureStage::ReadyForTurn(future), Suspend),
        };

        let turn = self.game.as_ref().unwrap().turn_state.clone();
        let turn_msg = Msg::Turn { turn: turn };

        let turn_future = ClientsSend::<String, C, BoxedFuture<C, ()>>::new(players,
                                                                            turn_msg.clone())
            .and_then(move |players| {
                ClientsSend::<String, C, BoxedFuture<C, ()>>::new(spectators, turn_msg)
                    .map(|spectators| (players, spectators))
            })
            .boxed();
        return (StartTurn(turn_future), Continue);
    }

    fn start_turn(&mut self,
                  mut future: BoxedFuture<(HashMap<String, C>, HashMap<String, C>), ()>)
                  -> GameFuturePollReturn<C> {
        let (mut players, spectators) = match future.poll() {
            Ok(Async::Ready(pair)) => pair,
            _ => return (GameFutureStage::StartTurn(future), Suspend),
        };
        self.spectators = Some(spectators);

        let turn = self.game.as_ref().unwrap().turn_state.clone();
        let (living_players, dead_players) = players.drain()
            .partition(|&(ref name, _)| turn.snakes.contains_key(name));
        self.players = Some(dead_players);

        let move_future =
            ClientsTimedReceive::<String, C, BoxedFuture<(Msg, C), ()>>::new(living_players,
                                                                             self.timeout,
                                                                             &self.timer)
                .boxed();
        return (GameFutureStage::AskMoves(move_future), Continue);
    }

    fn ask_moves(&mut self,
                 mut future: BoxedFuture<(HashMap<String, Msg>, HashMap<String, C>), ()>)
                 -> GameFuturePollReturn<C> {
        let (moves, living_players) = match future.poll() {
            Ok(Async::Ready((moves, players))) => (moves, players),
            _ => return (GameFutureStage::AskMoves(future), Suspend),
        };
        self.players.as_mut().unwrap().extend(living_players.into_iter());

        return (GameFutureStage::AdvanceTurn(moves), Continue);
    }

    fn advance_turn(&mut self, mut moves: HashMap<String, Msg>) -> GameFuturePollReturn<C> {
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
        let die_future =
            ClientsSend::<String, C, BoxedFuture<C, ()>>::new(casualty_players, Msg::Died).boxed();
        return (GameFutureStage::NotifyDead(die_future), Continue);
    }

    fn notify_dead(&mut self,
                   mut future: BoxedFuture<HashMap<String, C>, ()>)
                   -> GameFuturePollReturn<C> {
        let casualty_players = match future.poll() {
            Ok(Async::Ready(players)) => players,
            _ => return (GameFutureStage::NotifyDead(future), Suspend),
        };
        self.players.as_mut().unwrap().extend(casualty_players.into_iter());

        return (GameFutureStage::LoopDecision, Continue);
    }

    fn loop_decision(&mut self) -> GameFuturePollReturn<C> {
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

impl<C, R> Future for GameFuture<C, R>
    where C: Sink<SinkItem = ClientFutureCommand<String>, SinkError = ()> + Send + 'static,
          R: Rng
{
    type Item = (Game<R>, HashMap<String, C>, HashMap<String, C>);
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
