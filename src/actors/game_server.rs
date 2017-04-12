use futures::Future;
use kabuki::{Actor, ActorRef};
use rand::Rng;

use net::*;
use state::*;
use engine::*;
use utils::*;
use super::*;

pub type GameServerActorRef = ActorRef<<GameActor as Actor>::Request,
                                       <GameActor as Actor>::Response,
                                       <GameActor as Actor>::Error>;

#[derive(Clone)]
pub struct GameServerActor<F>
    where F: Fn() -> Box<Rng>
{
    rng_fn: F,
    grid: Grid,
    timeout: Milliseconds,
    game_actor_ref: GameActorRef,
}

impl<F> GameServerActor<F>
    where F: Fn() -> Box<Rng>
{
    pub fn new(rng_fn: F, grid: Grid, timeout: Milliseconds, game_actor_ref: GameActorRef) -> Self {
        GameServerActor {
            rng_fn: rng_fn,
            grid: grid,
            timeout: timeout,
            game_actor_ref: game_actor_ref,
        }
    }
}

impl<F> Actor for GameServerActor<F>
    where F: Fn() -> Box<Rng>
{
    type Request = MsgRoom<String>;
    type Response = (Game, MsgRoom<String>);
    type Error = ();
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&mut self, players: Self::Request) -> Self::Future {
        let rng = (self.rng_fn)();
        let grid = self.grid.clone();
        let game = Game::new(rng, grid);
        Box::new(self.game_actor_ref.call((game, players, self.timeout)))
    }
}
