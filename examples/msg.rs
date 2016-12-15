extern crate ansi_term;
extern crate sirpent;
extern crate serde_json;

use ansi_term::Colour::*;

use sirpent::*;

fn main() {
    println!("{}", Yellow.bold().paint("Sirpent"));

    println!("{:?}", serde_json::to_string(&VersionMsg::new()));
}
