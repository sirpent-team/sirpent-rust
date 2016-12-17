extern crate ansi_term;
extern crate sirpent;
extern crate rand;

use ansi_term::Colour::*;
use std::thread;
use std::str;
use std::time;
use std::io::{Error, ErrorKind};
use std::net::TcpStream;
use std::io::Result;
use std::sync::{Arc, RwLock};
use std::ops::Deref;
use rand::os::OsRng;
use std::collections::HashMap;

use sirpent::*;

fn main() {
    println!("{}", Yellow.bold().paint("Sirpent"));

    let grid = Grid { radius: 15 };
    let mut cell_counts = HashMap::new();

    for _ in 0..(0xFFFFFFF as usize) {
        let mut osrng = OsRng::new().unwrap();
        let random_cell = grid.random_cell(&mut osrng);
        let cell_count = cell_counts.entry(random_cell).or_insert(0);
        *cell_count += 1;
    }

    for (key, value) in cell_counts.iter() {
        println!("{:?} {:?}", key, value);
    }

    //println!("{:?}", cell_counts);
}
