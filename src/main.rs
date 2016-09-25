#![feature(custom_derive, plugin)]
#![plugin(serde_macros)]

extern crate ansi_term;
extern crate sirpent;
extern crate rand;
extern crate uuid;
extern crate serde;
extern crate serde_json;
#[cfg(test)]
extern crate quickcheck;

use ansi_term::Colour::*;
use uuid::Uuid;
use std::collections::HashMap;

use sirpent::grid::*;
use sirpent::hexagon_grid::*;
// use sirpent::square_grid::*;
// use sirpent::triangle_grid::*;
use sirpent::snake::*;
use sirpent::player::*;

#[derive(PartialEq, Eq, Clone, Hash, Debug, Serialize, Deserialize)]
struct SirpentLabel {
    pub version: String,
    pub msg_type: String,
}

impl SirpentLabel {
    pub fn new(msg_type: String) -> SirpentLabel {
        SirpentLabel {
            version: env!("CARGO_PKG_VERSION").to_string(),
            msg_type: msg_type,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Game<V: Vector> {
    pub world: World,
    pub players: HashMap<String, Player>,
    pub state: GameState<V>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GameState<V: Vector> {
    pub food: V,
    pub snakes: HashMap<Uuid, Snake<V>>,
}

#[derive(Serialize, Deserialize)]
struct RequestMove<V: Vector> {
    pub sirpent: SirpentLabel,
    pub world: World,
    pub player: Player,
    pub state: GameState<V>,
}

impl<V: Vector> RequestMove<V> {
    pub fn new(world: World, player: Player, game_state: GameState<V>) -> RequestMove<V> {
        RequestMove::<V> {
            sirpent: SirpentLabel::new("request_move".to_string()),
            world: world,
            player: player,
            state: game_state,
        }
    }
}

fn tick<V: Vector>(game: Game<V>) {
    for (player_name, player) in game.players {
        println!("Ticking on Player name={}", player_name);

        let request_move = RequestMove::<V>::new(game.world, player, game.state.clone());
        println!("request_move json={}",
                 serde_json::to_string_pretty(&request_move).unwrap());
        // player.send(request_move);
        // player.recv(player_move);
    }
}

fn main() {
    println!("{}", Yellow.bold().paint("Sirpent"));

    let world = World::HexagonGrid(HexagonGrid { radius: 5 });
    let state = GameState::<HexagonVector> {
        food: HexagonVector { x: 9, y: 13 },
        snakes: HashMap::new(),
    };
    let mut game = Game {
        world: world,
        players: HashMap::new(),
        state: state,
    };

    let segments = vec![HexagonVector { x: 3, y: 8 }];
    let snake = Snake::new(segments);
    let player = Player::new("abserde".to_string(), snake.uuid.clone());

    game.players.insert(player.name.clone(), player);
    game.state.snakes.insert(snake.uuid.clone(), snake);

    tick::<HexagonVector>(game);
}
