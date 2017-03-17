use rand::Rng;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use futures::{future, Async, BoxFuture, Future, Poll};

use super::*;
use net::*;
use utils::*;

enum GameFutureStage {
    StartOfGame,
    ReadyForRound(BoxFuture<(HashMap<ClientId, ClientStatus>, HashMap<ClientId, ClientStatus>),
                            ()>),
    StartRound(BoxFuture<(HashMap<ClientId, ClientStatus>, HashMap<ClientId, ClientStatus>), ()>),
    AskMoves(BoxFuture<(HashMap<ClientId, ClientStatus>, HashMap<ClientId, Msg>), ()>),
    AdvanceRound(HashMap<ClientId, Msg>),
    LoopDecision,
    Concluding(BoxFuture<(HashMap<ClientId, ClientStatus>, HashMap<ClientId, ClientStatus>), ()>),
    EndOfGame,
}

enum GameFutureStageControl {
    Continue,
    Suspend,
}

pub struct GameFuture<R>
    where R: Rng
{
    game: Option<Game<R>>,
    all_players: Room,
    living_players: Room,
    spectators: Arc<Mutex<Room>>,
    current_stage: Option<GameFutureStage>,
    timeout: Option<Milliseconds>,
}

use self::GameFutureStage::*;
use self::GameFutureStageControl::*;

type GameFuturePollReturn = (GameFutureStage, GameFutureStageControl);

impl<R> GameFuture<R>
    where R: Rng
{
    pub fn new(mut game: Game<R>,
               players: Room,
               spectators: Arc<Mutex<Room>>,
               timeout: Option<Milliseconds>)
               -> Self {
        for name in players.client_names() {
            if let Some(name) = name {
                game.add_player(name.clone());
            }
        }

        GameFuture {
            game: Some(game),
            all_players: players.clone(),
            living_players: players,
            spectators: spectators,
            current_stage: Some(StartOfGame),
            timeout: timeout,
        }
    }

    fn all_players(&mut self) -> &mut Room {
        &mut self.all_players //.expect("Players must be present.")
    }

    fn living_players(&mut self) -> &mut Room {
        &mut self.living_players //.expect("Players must be present.")
    }

    // fn spectators(&mut self) -> MutexGuard<Room> {
    //     self.spectators.lock().unwrap() //.expect("Spectators must be present.").lock().unwrap()
    // }

    fn start_of_game(&mut self) -> GameFuturePollReturn {
        let game = self.game.as_ref().unwrap().game_state.clone();
        let new_game_msg = Msg::Game { game: game };

        let players_txing = self.all_players().broadcast(new_game_msg.clone());
        let spectators_txing = self.spectators.lock().unwrap().broadcast(new_game_msg);
        let new_game_future = players_txing.join(spectators_txing).boxed();
        (ReadyForRound(new_game_future), Continue)
    }

    fn ready_for_round(&mut self,
                       mut future: BoxFuture<(HashMap<ClientId, ClientStatus>,
                                              HashMap<ClientId, ClientStatus>),
                                             ()>)
                       -> GameFuturePollReturn {
        let (player_statuses, spectator_statuses) = match future.poll() {
            Ok(Async::Ready(v)) => v,
            _ => return (ReadyForRound(future), Suspend),
        };

        let round = self.game.as_ref().unwrap().round_state.clone();
        let game_uuid = self.game.as_ref().unwrap().game_state.uuid;
        let round_msg = Msg::Round {
            round: round,
            game_uuid: game_uuid,
        };

        let players_txing = self.all_players().broadcast(round_msg.clone());
        let spectators_txing = self.spectators.lock().unwrap().broadcast(round_msg);
        let round_future = players_txing.join(spectators_txing).boxed();
        (StartRound(round_future), Continue)
    }

    fn start_round(&mut self,
                   mut future: BoxFuture<(HashMap<ClientId, ClientStatus>,
                                          HashMap<ClientId, ClientStatus>),
                                         ()>)
                   -> GameFuturePollReturn {
        let (player_statuses, spectator_statuses) = match future.poll() {
            Ok(Async::Ready(v)) => v,
            _ => return (ReadyForRound(future), Suspend),
        };

        let rxing_timeout = ClientTimeout::keep_alive_after(self.timeout.map(|m| *m));
        let living_players_rxing = self.living_players().receive(rxing_timeout).boxed();
        (AskMoves(living_players_rxing), Continue)
    }

    fn ask_moves(&mut self,
                 mut future: BoxFuture<(HashMap<ClientId, ClientStatus>, HashMap<ClientId, Msg>),
                                       ()>)
                 -> GameFuturePollReturn {
        let (living_player_statuses, msgs) = match future.poll() {
            Ok(Async::Ready(v)) => v,
            _ => return (AskMoves(future), Suspend),
        };
        (AdvanceRound(msgs), Continue)
    }

    fn advance_round(&mut self, mut moves: HashMap<ClientId, Msg>) -> GameFuturePollReturn {
        let directions = moves.drain()
            .filter_map(|(id, msg)| if let Msg::Move { direction } = msg {
                Some((self.all_players.name_of(&id).unwrap().clone(), Ok(direction)))
            } else {
                None
            })
            .collect();
        self.game.as_mut().unwrap().advance_round(directions);

        let new_round = &self.game.as_ref().unwrap().round_state;
        println!("Advanced round to {:?}", new_round.clone());

        (LoopDecision, Continue)
    }

    fn loop_decision(&mut self) -> GameFuturePollReturn {
        if self.game.as_ref().unwrap().concluded() {
            let round = self.game.as_ref().unwrap().round_state.clone();
            let game_uuid = self.game.as_ref().unwrap().game_state.uuid;
            let game_over_msg = Msg::outcome(round, game_uuid);

            let players_txing = self.all_players().broadcast(game_over_msg.clone());
            let spectators_txing = self.spectators.lock().unwrap().broadcast(game_over_msg);
            let concluding_future = players_txing.join(spectators_txing).boxed();
            (Concluding(concluding_future), Continue)
        } else {
            // While this type needs superceding, this empty future is a particular codesmell.
            (ReadyForRound(future::ok((HashMap::new(), HashMap::new())).boxed()), Continue)
        }
    }

    fn conclude(&mut self,
                mut future: BoxFuture<(HashMap<ClientId, ClientStatus>,
                                       HashMap<ClientId, ClientStatus>),
                                      ()>)
                -> GameFuturePollReturn {
        let (player_statuses, spectator_statuses) = match future.poll() {
            Ok(Async::Ready(pair)) => pair,
            _ => return (StartRound(future), Suspend),
        };
        (EndOfGame, Continue)
    }
}

impl<R> Future for GameFuture<R>
    where R: Rng
{
    type Item = (Game<R>, Room, Arc<Mutex<Room>>);
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
                    let all_players = self.all_players.clone();
                    let spectators = self.spectators.clone();
                    let return_triple = (game, all_players, spectators);
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
