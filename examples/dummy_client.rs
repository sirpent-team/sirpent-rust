#![feature(question_mark)]

extern crate ansi_term;
extern crate sirpent;
extern crate rand;

use ansi_term::Colour::*;
use std::net::TcpStream;
use std::str;

use sirpent::*;

fn main() {
    println!("{}", Yellow.bold().paint("Sirpent dummy-client example"));
    client_detect_vector();
}

pub fn client_detect_vector() {
    let stream = TcpStream::connect("127.0.0.1:5513").expect("Could not connect to server.");
    let mut player_connection = PlayerConnection::new(stream, None)
        .expect("Could not produce new PlayerConnection.");

    match player_connection.read().expect("Could not read anything; expected Command::Version.") {
        Command::Version { sirpent, protocol } => {
            println!("{:?}",
                     Command::Version {
                         sirpent: sirpent,
                         protocol: protocol,
                     })
        }
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    };

    let (grid, timeout) = match player_connection.read()
        .expect("Could not read anything; expected Command::Server.") {
        Command::Server { grid, timeout } => {
            println!("{:?}",
                     Command::Server {
                         grid: grid,
                         timeout: timeout,
                     });
            (grid, timeout)
        }
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    };

    player_connection.write(&Command::Hello {
            player: Player {
                name: "daenerys".to_string(),
                snake: None,
            },
            secret: Some("DeagOLmol3105764438410301265454621913800982laskhdasdj".to_string()),
        })
        .expect("Could not write Command::Hello.");

    match player_connection.read().expect("Could not read anything; expected Command::NewGame.") {
        Command::NewGame => println!("{:?}", Command::NewGame),
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    }

    let mut turn_game: Game = match player_connection.read()
        .expect("Could not read anything; expected Command::Turn.") {
        Command::Turn { game } => {
            println!("{:?}", Command::Turn { game: game.clone() });
            game
        }
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    };

    loop {
        println!("{:?}", turn_game);

        match player_connection.read()
            .expect("Could not read anything; expected Command::MakeAMove.") {
            Command::MakeAMove => println!("{:?}", Command::MakeAMove),
            command => {
                player_connection.write(&Command::Error).unwrap_or(());
                panic!(format!("Unexpected {:?}.", command));
            }
        }

        player_connection.write(&Command::Move { direction: Direction::variants()[0] })
            .expect("Could not write Command::Move.");

        match player_connection.read()
            .expect("Could not read anything; expected Command::Timedout/Died/Won/Turn.") {
            Command::TimedOut => {
                println!("{:?}", Command::TimedOut);
                return;
            }
            Command::Died => {
                println!("{:?}", Command::Died);
                return;
            }
            Command::Won => {
                println!("{:?}", Command::Won);
                return;
            }
            Command::Turn { game } => {
                println!("{:?}", Command::Turn { game: game.clone() });
                turn_game = game;
                continue;
            }
            command => {
                player_connection.write(&Command::Error).unwrap_or(());
                panic!(format!("Unexpected {:?}.", command));
            }
        }
    }
}
