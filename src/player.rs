use std::time::Duration;

use net::*;
use grid::*;
use snake::*;
use state::*;
use protocol::*;

pub type PlayerName = String;
pub type Move = Result<Direction, ProtocolError>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub name: PlayerName,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause_of_death: Option<CauseOfDeath>,
}

impl Player {
    pub fn new(name: PlayerName) -> Player {
        Player {
            name: name,
            cause_of_death: None,
        }
    }
}

#[derive(Debug)]
pub struct PlayerConnection {
    // pub state: PlayerState,
    pub conn: ProtocolConnection,
}

impl PlayerConnection {
    // @TODO: Convert to a From implementation.
    pub fn new(conn: ProtocolConnection) -> PlayerConnection {
        PlayerConnection { conn: conn }
    }

    pub fn version(&mut self) -> ProtocolResult<()> {
        self.conn.send(&Message::from_message_typed(VersionMsg::new()))
    }

    pub fn identify(&mut self) -> ProtocolResult<PlayerName> {
        let ident: ProtocolResult<IdentifyMsg> = self.conn.recieve()?.to_message_typed();
        match ident {
            Ok(IdentifyMsg { desired_player_name }) => Ok(desired_player_name),
            Err(e) => Err(e),
        }
    }

    pub fn welcome(&mut self, player_name: PlayerName, grid: Grid) -> ProtocolResult<()> {
        let read_timeout = self.conn.timeouts.read.clone();
        self.conn.send(&Message::from_message_typed(WelcomeMsg {
            player_name: player_name,
            grid: grid,
            timeout: read_timeout,
        }))
    }

    pub fn tell_new_game(&mut self, game_state: GameState) -> ProtocolResult<()> {
        self.conn.send(&Message::from_message_typed(NewGameMsg { game: game_state }))
    }

    pub fn tell_turn(&mut self, turn_state: TurnState) -> ProtocolResult<()> {
        self.conn.send(&Message::from_message_typed(TurnMsg { turn: turn_state }))
    }

    pub fn ask_next_move(&mut self) -> ProtocolResult<Direction> {
        let move_: ProtocolResult<MoveMsg> = self.conn.recieve()?.to_message_typed();
        match move_ {
            Ok(MoveMsg { direction }) => Ok(direction),
            Err(e) => Err(e),
        }
    }

    pub fn tell_death(&mut self, cause_of_death: CauseOfDeath) -> ProtocolResult<()> {
        self.conn.send(&Message::from_message_typed(DiedMsg { cause_of_death: cause_of_death }))
    }

    pub fn tell_won(&mut self, cause_of_death: CauseOfDeath) -> ProtocolResult<()> {
        self.conn.send(&Message::from_message_typed(WonMsg {}))
    }
}

#[derive(Debug)]
enum PlayerState {
    New,
    Version,
    Identify { desired_player_name: PlayerName },
    Ready,
    Playing,
    Errored(ProtocolError),
}

#[derive(Debug, Clone, PartialEq)]
enum PlayerEvent {
    Versioning,
    Identifying,
    Welcoming {
        player_name: PlayerName,
        grid: Grid,
        timeout: Option<Duration>,
    },
    GameBegins,
    GameEnds,
}

impl PlayerState {
    pub fn next(self, connection: &mut PlayerConnection, event: PlayerEvent) -> PlayerState {
        let a: ProtocolResult<PlayerState> = match (self, event) {
                (PlayerState::New, PlayerEvent::Versioning) => {
                    connection.version().and(Ok(PlayerState::Version))
                }
                (PlayerState::Version, PlayerEvent::Identifying) => {
                    connection.identify().and_then(|desired_player_name| {
                        Ok(PlayerState::Identify { desired_player_name: desired_player_name })
                    })
                }
                (PlayerState::Identify { ref desired_player_name },
                 PlayerEvent::Welcoming { ref player_name, grid, timeout }) => {
                    connection.welcome(player_name.clone(), grid)
                        .and(Ok(PlayerState::Ready))
                }
                (PlayerState::Ready, PlayerEvent::GameBegins) => Ok(PlayerState::Playing),
                (PlayerState::Playing, PlayerEvent::GameEnds) => Ok(PlayerState::Ready),
                (PlayerState::Errored(e), _) => Err(e),
                _ => unimplemented!(),
            }
            .or_else(|e| Ok(PlayerState::Errored(e)));
        a.unwrap()
    }
}
