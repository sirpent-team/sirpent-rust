#![feature(question_mark)]

extern crate ansi_term;
extern crate sirpent;
extern crate rand;

use ansi_term::Colour::*;
use std::net::TcpStream;
use std::str;
use std::time::Duration;

use sirpent::*;

fn main() {
    println!("{}", Yellow.bold().paint("Sirpent dummy-client example"));
    client_detect_vector::<HexagonVector>();
}

pub fn client_detect_vector<V: Vector>()
    where <V as sirpent::Vector>::Direction: 'static
{
    let stream = TcpStream::connect("127.0.0.1:5513").unwrap();
    let mut player_connection = PlayerConnection::<V>::new(stream).unwrap();

    let r = player_connection.read().unwrap();
    match r {
        Command::Version { sirpent, protocol } => {
            println!("{:?}",
                     Command::Version::<V> {
                         sirpent: sirpent,
                         protocol: protocol,
                     })
        }
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    };

    let r = player_connection.read().unwrap();
    match r {
        Command::Server { world, timeout } => {
            println!("{:?}",
                     Command::Server::<V> {
                         world: world,
                         timeout: timeout,
                     });
            let g = world.unwrap().0;
            let t = timeout;
            client::<g::Vector>(player_connection, g, t);
        }
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    };
}

pub fn client<V: Vector>(player_connection: PlayerConnection<V>, world: World, timeout: Option<Duration>)
    where <V as sirpent::Vector>::Direction: 'static
{
    player_connection.write(&Command::Hello {
            player: Player {
                name: "daenerys".to_string(),
                secret: Some("DeagOLmol3105764438410301265454621913800982laskhdasdj".to_string()),
                snake_uuid: None,
            },
        })
        .unwrap();

    match player_connection.read().unwrap() {
        Command::NewGame => println!("{:?}", Command::NewGame::<V>),
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    }

    let mut turn_game: Game<V> = match player_connection.read().unwrap() {
        Command::Turn { game } => {
            println!("{:?}", Command::Turn::<V> { game: game.clone() });
            game
        }
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    };

    loop {
        match player_connection.read().unwrap() {
            Command::MakeAMove => println!("{:?}", Command::MakeAMove::<V>),
            command => {
                player_connection.write(&Command::Error).unwrap_or(());
                panic!(format!("Unexpected {:?}.", command));
            }
        }

        player_connection.write(&Command::Move { direction: V::Direction::variants()[0] })
            .unwrap();

        match player_connection.read().unwrap() {
            Command::TimedOut => {
                println!("{:?}", Command::TimedOut::<V>);
                return;
            }
            Command::Died => {
                println!("{:?}", Command::Died::<V>);
                return;
            }
            Command::Won => {
                println!("{:?}", Command::Won::<V>);
                return;
            }
            Command::Turn { game } => {
                println!("{:?}", Command::Turn::<V> { game: game.clone() });
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
