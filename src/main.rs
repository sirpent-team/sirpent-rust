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
use std::net::SocketAddr;

use sirpent::grid::*;
use sirpent::hexagon_grid::*;
//use sirpent::square_grid::*;
//use sirpent::triangle_grid::*;
use sirpent::snake::*;
use sirpent::player::*;

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
struct SirpentLabel {
    pub version: String,
    pub msg_type: String,
}

impl SirpentLabel {
    pub fn new(msg_type: String) -> SirpentLabel {
        SirpentLabel{
            version: env!("CARGO_PKG_VERSION").to_string(),
            msg_type: msg_type,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, Serialize, Deserialize)]
struct Game<V : Vector> {
    pub world: World,
    pub players: HashMap<String, Player>,
    pub state: GameState<V>,
}

#[derive(Serialize, Deserialize)]
struct GameState<V : Vector> {
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

impl<V : Vector> RequestMove<V> {
    pub fn new(world: World, player: Player, game_state: GameState<V>) -> RequestMove<V> {
        RequestMove::<V>{
            sirpent: SirpentLabel::new("request_move".to_string()),
            world: world,
            player: player,
            state: game_state,
        }
    }
}

fn tick<V: Vector>(game: Game<V>) {
    for (player_name, player) in &game.players {
        println!("{} {}", player_name, player.server_address.unwrap());
        let request_move = RequestMove::<V>{
            sirpent: SirpentLabel::new("request_move".to_string()),
            world: game.world,
            player: player,
            state: game.state,
        };
        player.send(request_move);
        player.recv(player_move);
    }
}

fn main() {
    println!("{}", Yellow.bold().paint("Sirpent"));

    let world = World::HexagonGrid(HexagonGrid{radius: 5});
    let state = GameState::<HexagonVector>{
        food: HexagonVector{x: 9, y: 13},
        snakes: HashMap::new(),
    };
    let mut game = Game{
        world: world,
        players: HashMap::new(),
        state: state,
    };

    let segments = vec![HexagonVector{x: 3, y: 8}];
    let snake = Snake::<HexagonVector>::new(segments);
    let server_address: SocketAddr = "127.0.0.1:3535".parse().expect("Unable to parse socket address.");
    let player = Player::new("abserde".to_string(), server_address, snake.uuid.clone());

    game.players.insert(player.name.clone(), player);
    game.state.snakes.insert(snake.uuid.clone(), snake);

    tick::<HexagonVector>(game);

    //let rm = RequestMove::<HexagonVector>::new(world, player, state);
    //println!("{}", serde_json::to_string_pretty(&rm).unwrap());

    //player.connect(None).expect("Connection to player failed.");

    //game::<HexagonVector>(w, HexagonVector{x: 0, y: 0});
}
