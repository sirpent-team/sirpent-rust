extern crate ansi_term;
extern crate sirpent;
extern crate rand;
extern crate uuid;
#[cfg(test)]
extern crate quickcheck;

use ansi_term::Colour::*;
use std::str::FromStr;
use std::net;
use uuid::Uuid;

use sirpent::grid::*;
use sirpent::hexagon_grid::*;
use sirpent::square_grid::*;
use sirpent::triangle_grid::*;
use sirpent::snake::*;
use sirpent::player::*;

fn main() {
    println!("{}", Yellow.bold().paint("Sirpent"));

    let snake : Snake<HexagonVector>;
    snake = Snake {
        dead : false,
        uuid : Uuid::nil(),
        segments : vec!()
    };
    println!("{}", snake.is_head_at(&HexagonVector{x : 10, y : 10}));

    let server_address = net::SocketAddr::from_str("127.0.0.1:3001").expect("Invalid Socket Address");
    let mut player = Player::new(String::from("p1"), server_address);
    player.connect(None).expect("Connection to player failed.");
}
