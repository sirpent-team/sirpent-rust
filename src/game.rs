use grid::*;
use player::*;
use snake::*;

use rand::thread_rng;

use std::borrow::BorrowMut;
use std::boxed::Box;
use std::collections::HashSet;

pub struct Game<G : Grid> {
    grid : G,
    players : Vec<(Player<G>, Snake<G::Vector>)>
}

impl<G : Grid> Game<G> {
    pub fn new() -> Game<G> {
        Game{grid : G::new(20), players : Vec::new()}
    }
    pub fn add_player(&mut self, player : Player<G>) {
        let snake : Snake<G::Vector>;
        loop {
            if let Some(candidate) = self.grid.add_snake_at(G::Vector::rand_within(&mut thread_rng(), 20)) {
                snake = candidate;
                break;
            }
        }
        self.players.push((player, snake));
    }
    pub fn step(&mut self) {
        // poll directions and update snakes
        let mut new_head_positions = Vec::new();
        for &mut (ref mut player, ref mut snake) in self.players.iter_mut() {
            let dir = player.get_move(self.grid.get_local_map(snake.segments[0]));
            snake.step_in_direction(dir);
            new_head_positions.push((snake.segments[0], snake.uuid));
        }
        // check for collisions
        let mut to_remove = HashSet::new();
        // // check for head collisions
        for i in 0..new_head_positions.len() {
            for j in i + 1 .. new_head_positions.len() {
                if new_head_positions[i].0 == new_head_positions[j].0 {
                    to_remove.insert(new_head_positions[i].1);
                    to_remove.insert(new_head_positions[j].1);
                }
            }
        }
        // // check for collisions with the tail
        for i in 0..new_head_positions.len() {
            if let Some(x) = self.grid.get_cell_at(new_head_positions[i].0) {
            }
        }
        
        
        // update grid   

    }
}
