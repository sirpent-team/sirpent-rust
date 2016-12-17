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
    let mut protocol_connection = ProtocolConnection::new(stream, None)
        .expect("Could not produce new ProtocolConnection.");

    let version_msg: ProtocolResult<VersionMessage> = protocol_connection.recieve();
    match version_msg {
        Ok(VersionMessage { sirpent, protocol }) => {
            println!("{:?}",
                     VersionMessage {
                         sirpent: sirpent,
                         protocol: protocol,
                     })
        }
        Err(e) => {
            panic!(format!("Unexpected {:?}.", e));
        }
    };

    protocol_connection.send(IdentifyMessage { desired_player_name: "dummy-client".to_string() })
        .expect("Sending Identify.");

    let welcome_msg: ProtocolResult<WelcomeMessage> = protocol_connection.recieve();
    let (_, grid, _) = match welcome_msg {
        Ok(WelcomeMessage { player_name, grid, timeout }) => {
            println!("{:?}",
                     WelcomeMessage {
                         player_name: player_name.clone(),
                         grid: grid,
                         timeout: timeout,
                     });
            (player_name, grid, timeout)
        }
        Err(e) => {
            panic!(format!("Unexpected {:?}.", e));
        }
    };

    let new_game_msg: ProtocolResult<NewGameMessage> = protocol_connection.recieve();
    match new_game_msg {
        Ok(NewGameMessage { game }) => println!("{:?}", NewGameMessage { game: game }),
        Err(e) => panic!(format!("Unexpected {:?}.", e)),
    }

    loop {
        let turn_msg: ProtocolResult<TurnMessage> = protocol_connection.recieve();
        match turn_msg {
            Ok(TurnMessage { turn }) => println!("{:?}", TurnMessage { turn: turn }),
            Err(e) => panic!(format!("Unexpected {:?}.", e)),
        }

        protocol_connection.send(MoveMessage { direction: Direction::variants()[0] })
            .expect("Sending Move.");
    }
}
