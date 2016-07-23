extern crate rand;
extern crate uuid;
#[cfg(test)]
extern crate quickcheck;

mod grid;
mod hexgrid;
mod snake;
mod player;
mod game;

use uuid::Uuid;

use hexgrid::*;
use snake::*;

fn main() {
    let snake : Snake<HexVector>;
    snake = Snake {
        growing : false,
        uuid : Uuid::nil(),
        segments : vec!()
    };
    println!("{}", snake.is_head_at(&HexVector{x : 10, y : 10}));
}
    
