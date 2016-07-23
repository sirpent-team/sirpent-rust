use std::collections::hash_map::{HashMap, RandomState};
use std::hash::Hash;

use rand::Rng;
use uuid::Uuid;

use snake::Snake;

#[derive(Hash, PartialEq, Eq, Clone, Copy)]
pub enum Cell {
    Segment(Uuid),
    Empty
}

pub trait Vector : Eq + Copy {
    type Direction;
    fn distance(&self, other : &Self) -> usize;
    fn subtract(&self, other : &Self) -> Self;
    fn neighbour(&self, direction : &Self::Direction) -> Self;
    fn ball_around(&self, radius : usize) -> Vec<Self>;
    fn rand_within<R : Rng>(rng : &mut R, radius : usize) -> Self;
}
    
pub trait Grid {
    type Vector : Vector;
    fn new(radius : usize) -> Self;
    fn dimensions(&self) -> Vec<isize>;
    fn is_within_bounds(&self, v : Self::Vector) -> bool;
    fn add_snake_at(&mut self, coord : Self::Vector) -> Option<Snake<Self::Vector>>;
    fn get_local_map(&self, location : Self::Vector) -> HashMap<Self::Vector, Cell, RandomState>;
    fn get_cell_at(&self, location : Self::Vector) -> Option<Cell>;
}
