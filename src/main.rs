extern crate ansi_term;
extern crate sirpent;
extern crate rand;
extern crate uuid;
extern crate rustc_serialize;
#[cfg(test)]
extern crate quickcheck;

use ansi_term::Colour::*;
use std::str::FromStr;
use std::net;
use uuid::Uuid;
use rustc_serialize::json;

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

    let g = HexagonGrid{radius: 5};
    println!("{:?}", grid_to_json(g));
    let g = HexagonGrid{radius: 5};
    let gs = Grids::HexagonGrid(g);
    println!("{:?}", grids_to_json(gs));

    let server_address = net::SocketAddr::from_str("127.0.0.1:3001").expect("Invalid Socket Address");
    let mut player = Player::new(String::from("p1"), server_address);
    player.connect(None).expect("Connection to player failed.");
}

fn grid_to_json<G: Grid>(grid: G) -> String {
    json::encode(&grid).unwrap()
}

fn grids_to_json(grid: Grids) -> String {
    json::encode(&grid).unwrap()
}

fn json_to_grid<G: Grid>(json: String) -> Grids {
    json::decode(&json).unwrap()
}

/*
pub fn encode_grid


fn do_encode<E: Encodable>(e: E) -> () {
    let mut buf = String::new();
    e.encode(&mut json::Encoder::new(&mut buf)).unwrap();
    println!("{}", buf);
}

fn main() {
    let is_valid = true;
    match is_valid {
        true => do_encode(Valid { value: 42 }),
        false => do_encode(Error { error: "bork" }),
    };
}


fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        s.emit_struct("Point", 2, |s| {
            try!(s.emit_struct_field("x", 0, |s| {
                s.emit_i32(self.x)
            }));
            try!(s.emit_struct_field("y", 1, |s| {
                s.emit_i32(self.y)
            }));
            Ok(())
        })
    }
*/
