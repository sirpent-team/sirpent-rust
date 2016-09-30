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
    client_detect_vector();
}

pub fn client_detect_vector() {
    let stream = TcpStream::connect("127.0.0.1:5513").unwrap();
    let mut player_connection = PlayerConnection::new(stream).unwrap();

    let r = player_connection.read().unwrap();
    match r {
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

    let r = player_connection.read().unwrap();
    match r {
        Command::Server { world, timeout } => {
            println!("{:?}",
                     Command::Server {
                         world: world,
                         timeout: timeout,
                     });
            let w = world.unwrap();
            let t = timeout;
            match w {
                World::HexagonGrid(hg) => {
                    client::<HexagonGrid>(player_connection as PlayerConnection<HexagonGrid>, w, t)
                }
                World::SquareGrid(sg) => {
                    client::<SquareGrid>(player_connection as PlayerConnection<SquareGrid>, w, t)
                }
                World::TriangleGrid(tg) => {
                    client::<TriangleGrid>(player_connection as PlayerConnection<TriangleGrid>,
                                           w,
                                           t)
                }
            };
        }
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    };
}

pub fn client<G: Grid>(player_connection: PlayerConnection<G>,
                       world: World,
                       timeout: Option<Duration>)
    where <<G as Grid>::Vector as Vector>::Direction: 'static
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
        Command::NewGame => println!("{:?}", Command::NewGame::<G>),
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    }

    let mut turn_game: Game<G::Vector> = match player_connection.read().unwrap() {
        Command::Turn { game } => {
            println!("{:?}", Command::Turn::<G> { game: game.clone() });
            game
        }
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    };

    loop {
        match player_connection.read().unwrap() {
            Command::MakeAMove => println!("{:?}", Command::MakeAMove::<G>),
            command => {
                player_connection.write(&Command::Error).unwrap_or(());
                panic!(format!("Unexpected {:?}.", command));
            }
        }

        player_connection.write(&Command::Move { direction: <<G as sirpent::Grid>::Vector as Vector>::Direction::variants()[0] })
            .unwrap();

        match player_connection.read().unwrap() {
            Command::TimedOut => {
                println!("{:?}", Command::TimedOut::<G>);
                return;
            }
            Command::Died => {
                println!("{:?}", Command::Died::<G>);
                return;
            }
            Command::Won => {
                println!("{:?}", Command::Won::<G>);
                return;
            }
            Command::Turn { game } => {
                println!("{:?}", Command::Turn::<G> { game: game.clone() });
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
