use grid::*;

use std::collections::hash_map::{HashMap, RandomState};
use std::marker::PhantomData;

pub struct Player<G : Grid> {
    phantom : PhantomData<G>

}

impl <G : Grid> Player<G> {
    pub fn get_move(&mut self, locale : HashMap<G::Vector, Cell, RandomState>) -> <G::Vector as Vector>::Direction{
        unimplemented!();
    }
    //fn signal_defeat(&mut self);
}
