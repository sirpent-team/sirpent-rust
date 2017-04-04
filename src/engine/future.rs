use rand::Rng;
use std::time::Duration;
use std::collections::HashMap;
use futures::{Future, Sink, Poll, Async, Stream};
use futures::sync::mpsc;
use tokio_timer::Timer;

use super::*;
use net::*;
use utils::*;

pub fn game_future<R>
    (mut game: Game<R>,
     players: Room<String, MsgTransport>,
     spectator_tx: mpsc::Sender<Msg>,
     timeout: Option<Milliseconds>,
     timer: Timer)
     -> Box<Future<Item = (Game<R>, Room<String, MsgTransport>, mpsc::Sender<Msg>), Error = ()>>
    where R: Rng + 'static
{
    let timeout = *timeout.unwrap();

    for id in players.ids() {
        game.add_player(id.clone());
    }

    let game_msg = Msg::Game { game: Box::new(game.game_state().clone()) };
    let future = players
        .broadcast_all(game_msg.clone())
        .and_then(move |players| {
            spectator_tx
                .send(game_msg)
                .map_err(|_| ())
                .and_then(move |spectator_tx| {
                    let rounds_future =
                        RoundsFuture::new(game, players, spectator_tx, timeout, timer);
                    rounds_future.and_then(|(game, players, spectator_tx)| {
                        let outcome_msg = Msg::outcome(game.round_state().clone(),
                                                       game.game_state().uuid);

                        players
                            .broadcast_all(outcome_msg.clone())
                            .and_then(|players| {
                                          spectator_tx
                                              .send(outcome_msg)
                                              .map_err(|_| ())
                                              .map(|spectator_tx| (game, players, spectator_tx))
                                      })
                    })
                })
        });
    Box::new(future)
}

pub struct RoundsFuture<R>
    where R: Rng + 'static
{
    rounds_stream: Option<RoundsStream<R>>,
}

impl<R> RoundsFuture<R>
    where R: Rng + 'static
{
    pub fn new(game: Game<R>,
               players: Room<String, MsgTransport>,
               spectator_tx: mpsc::Sender<Msg>,
               timeout: Duration,
               timer: Timer)
               -> RoundsFuture<R> {
        let rounds_stream = RoundsStream::new(game, players, spectator_tx, timeout, timer);
        RoundsFuture { rounds_stream: Some(rounds_stream) }
    }
}

impl<R> Future for RoundsFuture<R>
    where R: Rng + 'static
{
    type Item = (Game<R>, Room<String, MsgTransport>, mpsc::Sender<Msg>);
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let poll = {
            let rounds_stream = self.rounds_stream.as_mut().unwrap();
            rounds_stream.poll()
        };
        match poll {
            Ok(Async::Ready(None)) => {
                let rounds_stream = self.rounds_stream.take().unwrap();
                Ok(Async::Ready(rounds_stream.into_inner()))
            }
            Ok(Async::Ready(Some(_))) => self.poll(),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => Err(e),
        }
    }
}

pub struct RoundsStream<R>
    where R: Rng + 'static
{
    timeout: Duration,
    timer: Timer,
    round_future: Box<Future<Item = (Game<R>, Room<String, MsgTransport>, mpsc::Sender<Msg>),
                             Error = ()>>,
    // This screams that either this wants to be a Future or `round` must become a custom Future.
    // I'd prefer `round` a custom future (such that these things can be taken out of it) and yet
    // such a future could be vast.
    inner: Option<(Game<R>, Room<String, MsgTransport>, mpsc::Sender<Msg>)>,
}

impl<R> RoundsStream<R>
    where R: Rng + 'static
{
    pub fn new(game: Game<R>,
               players: Room<String, MsgTransport>,
               spectator_tx: mpsc::Sender<Msg>,
               timeout: Duration,
               timer: Timer)
               -> RoundsStream<R> {
        let round_future = round(game, players, spectator_tx, timeout, timer.clone());
        RoundsStream {
            timeout: timeout,
            timer: timer,
            round_future: round_future,
            inner: None,
        }
    }

    pub fn into_inner(mut self) -> (Game<R>, Room<String, MsgTransport>, mpsc::Sender<Msg>) {
        self.inner.take().unwrap()
    }
}

impl<R> Stream for RoundsStream<R>
    where R: Rng + 'static
{
    type Item = Vec<String>;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.round_future.poll() {
            Ok(Async::Ready((game, players, spectator_tx))) => {
                if game.concluded() {
                    self.inner = Some((game, players, spectator_tx));
                    Ok(Async::Ready(None))
                } else {
                    let living_player_ids = game.round_state().snakes.keys().cloned().collect();
                    self.round_future = round(game,
                                              players,
                                              spectator_tx,
                                              self.timeout,
                                              self.timer.clone());
                    Ok(Async::Ready(Some(living_player_ids)))
                }
            }
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => Err(e),
        }
    }
}

fn round<R>
    (mut game: Game<R>,
     players: Room<String, MsgTransport>,
     spectator_tx: mpsc::Sender<Msg>,
     _: Duration,
     _: Timer)
     -> Box<Future<Item = (Game<R>, Room<String, MsgTransport>, mpsc::Sender<Msg>), Error = ()>>
    where R: Rng + 'static
{
    let round_msg = Msg::Round {
        round: Box::new(game.round_state().clone()),
        game_uuid: game.game_state().uuid,
    };
    let fut = players
        .broadcast_all(round_msg.clone())
        .and_then(|players| {
            spectator_tx
                .send(round_msg)
                .map_err(|_| ())
                .and_then(|spectator_tx| {
                    let living_player_ids = game.round_state().snakes.keys().cloned().collect();
                    players
                        .receive(living_player_ids)
                        //.with_soft_timeout(timeout, &timer)
                        .map(|(msgs, players)| {
                                 let directions = msgs_to_directions(msgs);
                                 game.next(Event::Turn(directions));
                                 (game, players, spectator_tx)
                             })
                })
        });
    Box::new(fut)
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
