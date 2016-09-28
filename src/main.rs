#![feature(custom_derive, plugin)]
#![plugin(serde_macros)]

extern crate ansi_term;
extern crate sirpent;
extern crate rand;
extern crate uuid;
extern crate serde;
extern crate serde_json;
extern crate openssl;
#[cfg(test)]
extern crate quickcheck;

use ansi_term::Colour::*;
use uuid::Uuid;
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::io::{Write, Read, BufRead, BufReader, BufWriter};
use openssl::ssl::{SslContext, SslMethod, SslStream, SSL_VERIFY_NONE, Ssl, MaybeSslStream};
use openssl::x509::X509FileType;
use std::str;
use std::time;
use std::path::PathBuf;

use sirpent::*;

#[derive(PartialEq, Eq, Clone, Hash, Debug, Serialize, Deserialize)]
struct SirpentLabel {
    pub version: String,
    pub msg_type: String,
}

impl SirpentLabel {
    pub fn new(msg_type: String) -> SirpentLabel {
        SirpentLabel {
            version: env!("CARGO_PKG_VERSION").to_string(),
            msg_type: msg_type,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Game<V: Vector> {
    pub uuid: Uuid,
    pub world: World,
    pub players: HashMap<String, Player>,
    pub state: GameState<V>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GameState<V: Vector> {
    pub food: V,
    pub snakes: HashMap<Uuid, Snake<V>>,
}

#[derive(Serialize, Deserialize)]
struct RequestMove<V: Vector> {
    pub sirpent: SirpentLabel,
    pub player: Player,
    pub game: Game<V>,
}

impl<V: Vector> RequestMove<V> {
    pub fn new(player: Player, game: Game<V>) -> RequestMove<V> {
        RequestMove::<V> {
            sirpent: SirpentLabel::new("request_move".to_string()),
            player: player,
            game: game,
        }
    }
}

fn tick<V: Vector>(game: Game<V>) {
    // @TODO: Use lifetimes to avoid looping over Clone-d game.players, and cloning in general.
    for (player_name, player) in game.clone().players {
        println!("Ticking on Player name={}", player_name);

        let request_move = RequestMove::new(player, game.clone());
        println!("request_move json={}",
                 serde_json::to_string_pretty(&request_move).unwrap());
        // player.send(request_move);
        // player.recv(player_move);
    }
}

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

    tick::<HexagonVector>(game);

    // -----------------------------------------------------------------------

    thread::spawn(move || {
        let plain_server = SirpentServer::plain("0.0.0.0:5513").unwrap();
        plain_server.listen(&player_connection_handler, None)
    });

    // -----------------------------------------------------------------------

    let cert = PathBuf::from("cert.pem");
    let key = PathBuf::from("key.pem");
    thread::spawn(move || {
        let tls_server = SirpentServer::tls(cert, key, "0.0.0.0:5514").unwrap();
        tls_server.listen(&player_connection_handler, None)
    });

    // -----------------------------------------------------------------------

    thread::sleep(time::Duration::from_millis(500));
    thread::spawn(move || tell_player_to_unsecured());
    thread::sleep(time::Duration::from_millis(500));
    thread::spawn(move || tell_player_to_ssl());

    loop {
        thread::sleep(time::Duration::from_millis(500));
    }
}

// @TODO: Get a competent review of the decoding code, and move into a type-parametric
// read function.
fn player_connection_handler(mut stream: MaybeSslStream<TcpStream>,
                             mut reader: BufReader<MaybeSslStream<TcpStream>>,
                             mut writer: BufWriter<MaybeSslStream<TcpStream>>) {
    // Prevent memory exhaustion: stop reading from string after 1MiB.
    // @TODO @DEBUG: Need to reset this for each new message communication.
    // let mut take = reader.clone().take(0xfffff);
    //
    // let json: Player = serde_json::from_reader(reader).unwrap();
    // println!("{:?}", json);
    // writer.write_fmt(format_args!("abc")).unwrap();
    // writer.flush().unwrap();


    // Read ASCII-encoded length of JSON string to follow.
    let mut msg_len_buf = Vec::new();
    // @TODO: Don't panic.
    reader.read_until(b' ', &mut msg_len_buf).unwrap();
    // Remove trailing space.
    msg_len_buf.pop();
    // Convert to slice.
    let msg_len_buf = &msg_len_buf[..];
    // Decode nubmer.
    // @TODO: Don't panic.
    let msg_len = u64::from_str_radix(str::from_utf8(msg_len_buf).unwrap(), 10).unwrap();
    println!("{:?}", msg_len);

    if msg_len == 0 {
        return;
    }

    let mut take = reader.take(msg_len & 0xfffff);

    let json: Player = serde_json::from_reader(take).unwrap();
    println!("{:?}", json);

    // // Decode JSON into a Player.
    // let mut json_str = Vec::with_capacity((msg_len + 1) as usize);
    // let mut json_str_buf = &mut json_str[..];
    // @TODO: Ensure correct number of chars read.
    // let read_json_str_chars = reader.read_exact(&mut json_str_buf).unwrap();
    // println!("{:?}", json_str_buf);
    // @TODO: Don't panic.
    // let json: Player = serde_json::from_str(str::from_utf8(json_str_buf).unwrap()).unwrap();
    // println!("{:?}", json);
}

pub fn tell_player_to_unsecured() {
    let mut stream = TcpStream::connect("127.0.0.1:5513").unwrap();
    let s = b"56 {\"name\": \"drogon\", \"secret\": \"D50gOmol310982laskhdasdj\"}";
    let mut bw = BufWriter::new(stream);
    bw.write(s);
    bw.flush();
}

pub fn tell_player_to_ssl() {
    let mut stream = TcpStream::connect("127.0.0.1:5514").unwrap();
    let ssl = ssl_to_io(SslContext::new(SslMethod::Tlsv1)).unwrap();
    let mut ssl_stream = ssl_to_io(SslStream::connect(&ssl, stream)).unwrap();
    let s = b"56 {\"name\": \"drogon\", \"secret\": \"D50gOmol310982laskhdasdj\"}";
    let mut bw = BufWriter::new(ssl_stream);
    bw.write(s);
    bw.flush();
}
