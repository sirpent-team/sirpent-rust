#![feature(question_mark)]

extern crate ansi_term;
extern crate sirpent;
extern crate rand;
extern crate uuid;

use ansi_term::Colour::*;
use uuid::Uuid;
use std::collections::HashMap;
use std::net::TcpStream;
use std::thread;
use std::str;
use std::time;

use sirpent::*;

fn main() {
    println!("{}", Yellow.bold().paint("Sirpent"));

    let mut game = Game {
        uuid: Uuid::new_v4(),
        grid: Grid::hexagon(15),
        players: HashMap::new(),
        food: Vector::hexagon(9, 13),
    };

    let snake = Snake::new(vec![Vector::hexagon(3, 8)]);
    let player = Player::new("abserde".to_string(), None, Some(snake));
    game.players.insert(player.clone().name, player.clone());

    // -----------------------------------------------------------------------

    thread::spawn(move || {
        let plain_server = SirpentServer::plain("0.0.0.0:5513").unwrap();
        plain_server.listen(move |stream: TcpStream| server_handler(stream, game.clone()),
                            None)
    });

    // -----------------------------------------------------------------------

    loop {
        thread::sleep(time::Duration::from_millis(500));
    }
}

fn server_handler(stream: TcpStream, game: Game) {
    // Prevent memory exhaustion: stop reading from string after 1MiB.
    // @TODO @DEBUG: Need to reset this for each new message communication.
    // let mut take = reader.clone().take(0xfffff);

    let mut player_connection = PlayerConnection::new(stream)
        .expect("Could not produce new PlayerConnection.");

    player_connection.write(&Command::version()).expect("Could not write Command::version().");

    player_connection.write(&Command::Server {
            grid: Some(game.grid),
            timeout: None,
        })
        .expect("Could not write Command::Server.");

    match player_connection.read().expect("Could not read anything; expected Command::Hello.") {
        Command::Hello { player } => println!("{:?}", player),
        Command::Quit => {
            println!("QUIT");
            return;
        }
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    }

    player_connection.write(&Command::NewGame).expect("Could not write Command::NewGame.");

    player_connection.write(&Command::Turn { game: game })
        .expect("Could not write Command::Turn.");

    player_connection.write(&Command::MakeAMove).expect("Could not write Command::MakeAMove.");

    match player_connection.read().expect("Could not read anything; expected Command::Move.") {
        Command::Move { direction } => println!("{:?}", Command::Move { direction: direction }),
        Command::Quit => {
            println!("QUIT");
            return;
        }
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    }
}
