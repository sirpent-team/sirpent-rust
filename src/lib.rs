extern crate uuid;
extern crate rand;
#[cfg(test)] extern crate quickcheck;

pub mod grid;
pub mod hexagon_grid;
pub mod square_grid;
pub mod triangle_grid;
pub mod snake;
pub mod player;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
