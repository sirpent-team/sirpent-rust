#[cfg(test)]
extern crate quickcheck;

mod grid;
mod hexgrid;
mod snake;

use hexgrid::*;
use snake::*;

fn main() {
    let snake : Snake<HexGrid>;
    snake = Snake {
        dead : false,
        segments : vec!()
    };
    println!("{}", snake.is_head_at(&HexVector{x : 10, y : 10}));
}
    
