extern crate ansi_term;
extern crate sirpent;
extern crate serde_json;

use ansi_term::Colour::*;

use sirpent::*;

fn main() {
    println!("{}", Yellow.bold().paint("Sirpent"));

    println!("{:?}", serde_json::to_string(&VersionMessage::new()));
    println!("{:?}",
             serde_json::to_string(&PlainMessage::from_typed(VersionMessage::new())));

    let s = serde_json::to_string(&PlainMessage::from_typed(VersionMessage::new())).unwrap();
    let p: PlainMessage = serde_json::from_str(&*s).unwrap();
    println!("{:?}", p);
    let v: VersionMessage = p.to_typed().unwrap();
    println!("{:?}", v);
}
