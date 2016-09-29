#![feature(custom_derive, plugin, question_mark)]
#![plugin(serde_macros)]

extern crate ansi_term;
extern crate sirpent;
extern crate rand;
extern crate uuid;
extern crate serde;
extern crate serde_json;
#[cfg(test)]
extern crate quickcheck;

use ansi_term::Colour::*;
use uuid::Uuid;
use std::collections::HashMap;
use std::net::TcpStream;
use std::thread;
use std::io::{Result, Read, Write, BufReader, BufWriter, Bytes, Error, ErrorKind};
use std::str;
use std::time;
use std::result::Result as StdResult;
use std::error::Error as StdError;

use sirpent::*;

// fn tick<V: Vector>(game: Game<V>) {
// @TODO: Use lifetimes to avoid looping over Clone-d game.players, and cloning in general.
// for (player_name, player) in game.clone().players {
// println!("Ticking on Player name={}", player_name);
//
// let request_move = RequestMove::new(player, game.clone());
// println!("request_move json={}",
// serde_json::to_string_pretty(&request_move).unwrap());
// player.send(request_move);
// player.recv(player_move);
// }
// }

fn main() {
    println!("{}", Yellow.bold().paint("Sirpent"));

    let world = World::HexagonGrid(HexagonGrid { radius: 5 });
    let state = GameState {
        food: HexagonVector { x: 9, y: 13 },
        snakes: HashMap::new(),
    };
    let mut game = Game {
        uuid: Uuid::new_v4(),
        world: world,
        players: HashMap::new(),
        state: state,
    };

    let segments = vec![HexagonVector { x: 3, y: 8 }];
    let snake = Snake::new(segments);
    let player = Player::new("abserde".to_string(), None, snake.uuid.clone());

    game.players.insert(player.name.clone(), player);
    game.state.snakes.insert(snake.uuid.clone(), snake);

    // tick::<HexagonVector>(game);

    // -----------------------------------------------------------------------

    thread::spawn(move || {
        let plain_server = SirpentServer::plain("0.0.0.0:5513").unwrap();
        //plain_server.listen(&player_connection_handler::<HexagonVector>, None)
        plain_server.listen(move |stream: TcpStream| {
            player_connection_handler::<HexagonVector>(stream, game.clone())
        }, None)
    });

    // -----------------------------------------------------------------------

    thread::sleep(time::Duration::from_millis(500));
    thread::spawn(move || tell_player_to_unsecured::<HexagonVector>());

    loop {
        thread::sleep(time::Duration::from_millis(500));
    }
}

/// Converts a Result<T, serde_json::Error> into an Result<T>.
fn serde_to_io<T>(res: StdResult<T, serde_json::Error>) -> Result<T> {
    match res {
        Ok(x) => Ok(x),
        Err(e) => {
            Err(Error::new(ErrorKind::Other,
                           &format!("A serde_json error occurred. ({})", e.description())[..]))
        }
    }
}

// @TODO: Add Drop to PlayerConnection that sends QUIT? Potential for deadlock waiting if so?
pub struct PlayerConnection<V: Vector> {
    stream: TcpStream,
    reader: serde_json::StreamDeserializer<Command<V>, Bytes<BufReader<TcpStream>>>,
    writer: BufWriter<TcpStream>,
}

impl<V: Vector> PlayerConnection<V> {
    pub fn new(stream: TcpStream) -> Result<PlayerConnection<V>> {
        Ok(PlayerConnection {
            stream: stream.try_clone()?,
            reader: serde_json::StreamDeserializer::new(BufReader::new(stream.try_clone()?)
                .bytes()),
            writer: BufWriter::new(stream),
        })
    }

    pub fn read(&mut self) -> Result<Command<V>> {
        match serde_to_io(self.reader.next().unwrap()) {
            Ok(command) => Ok(command),
            Err(e) => {
                // @TODO: It seems irrelevant whether writing ERROR succeeded or not. If it
                // succeeds then wonderful; the other end might get to know something went wrong.
                // If it fails then we're much better off returning the Read error than the
                // extra-level-of-indirection Write error.
                self.write(&Command::Error).unwrap_or(());
                Err(e)
            }
        }
    }

    pub fn write(&mut self, command: &Command<V>) -> Result<()> {
        self.writer.write_all(serde_to_io(serde_json::to_string(command))?.as_bytes())?;
        self.writer.flush()?;
        Ok(())
    }
}

// @TODO: Get a competent review of the decoding code, and move into a type-parametric
// read function.
fn player_connection_handler<V: Vector>(stream: TcpStream, game: Game<V>) {
    // Prevent memory exhaustion: stop reading from string after 1MiB.
    // @TODO @DEBUG: Need to reset this for each new message communication.
    // let mut take = reader.clone().take(0xfffff);

    let mut player_connection = PlayerConnection::<V>::new(stream).unwrap();

    player_connection.write(&Command::version()).unwrap();

    player_connection.write(&Command::Server {
            world: None,
            timeout: None,
        })
        .unwrap();

    match player_connection.read().unwrap() {
        Command::Hello { player } => println!("{:?}", player),
        Command::Quit => {
            println!("QUIT");
            return
        },
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    }

    player_connection.write(&Command::NewGame).unwrap();

    player_connection.write(&Command::Turn {
        game: game
    }).unwrap();

    player_connection.write(&Command::MakeAMove).unwrap();

    match player_connection.read().unwrap() {
        Command::Move { direction } => println!("{:?}", Command::Move::<V> { direction: direction }),
        Command::Quit => {
            println!("QUIT");
            return
        },
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    }
}

pub fn tell_player_to_unsecured<V: Vector>()
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
    }

    let r = player_connection.read().unwrap();
    match r {
        Command::Server { world, timeout } => {
            println!("{:?}",
                     Command::Server::<V> {
                         world: world,
                         timeout: timeout,
                     })
        }
        command => {
            player_connection.write(&Command::Error).unwrap_or(());
            panic!(format!("Unexpected {:?}.", command));
        }
    }

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
        },
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
                return
            }
            Command::Died => {
                println!("{:?}", Command::Died::<V>);
                return
            },
            Command::Won => {
                println!("{:?}", Command::Won::<V>);
                return
            },
            Command::Turn { game } => {
                println!("{:?}", Command::Turn::<V> { game: game.clone() });
                turn_game = game;
                continue
            },
            command => {
                player_connection.write(&Command::Error).unwrap_or(());
                panic!(format!("Unexpected {:?}.", command));
            }
        }
    }
}
