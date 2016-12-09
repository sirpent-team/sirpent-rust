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
    pub fn handshake(&mut self, grid: Grid) -> Result<Player, ProtocolError> {
        self.conn.send(&Command::version())?;
        let read_timeout = self.conn.timeouts.read;
        self.conn
            .send(&Command::Welcome {
                grid: grid,
                timeout: read_timeout,
            })?;
        match self.conn.recieve() {
                Ok(Command::Identify { player }) => Ok(player),
                Ok(command) => Err(ProtocolError::WrongCommand { command: command }),
                Err(e) => Err(e),
            }
            .and_then(|player| {
                self.conn.send(&Command::Identified { player_name: player.name.clone() })?;
                Ok(player)
            })
    }

    pub fn tell_new_game(&mut self, game_state: GameState) -> Result<(), ProtocolError> {
        self.conn.send(&Command::NewGame { game: game_state })
    }

    pub fn tell_turn(&mut self, turn_state: TurnState) -> Result<(), ProtocolError> {
        self.conn.send(&Command::Turn { turn: turn_state })
    }

    pub fn ask_next_move(&mut self) -> Result<Direction, ProtocolError> {
        match self.conn.recieve() {
            Ok(Command::Move { direction }) => Ok(direction),
            Ok(command) => Err(ProtocolError::WrongCommand { command: command }),
            Err(e) => Err(e),
        }
    }

    pub fn tell_death(&mut self, cause_of_death: CauseOfDeath) -> Result<(), ProtocolError> {
        self.conn.send(&Command::Died { cause_of_death: cause_of_death })
    }

    pub fn tell_won(&mut self, cause_of_death: CauseOfDeath) -> Result<(), ProtocolError> {
        self.conn.send(&Command::Died { cause_of_death: cause_of_death })
    }
}
