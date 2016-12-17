use net::*;
use grid::*;
use snake::*;
use state::*;
use protocol::*;

pub type PlayerName = String;
// For the time being, Move needs to be Cloneable. As a result it uses CauseOfDeath instead
// of ProtocolError - the former being deliberately Clone itself.
pub type Move = Result<Direction, CauseOfDeath>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

    pub fn send_version(&mut self) -> ProtocolResult<()> {
        self.conn.send(VersionMessage::new())
    }

    pub fn recieve_identify(&mut self) -> ProtocolResult<PlayerName> {
        let ident: ProtocolResult<IdentifyMessage> = self.conn.recieve();
        match ident {
            Ok(IdentifyMessage { desired_player_name }) => Ok(desired_player_name),
            Err(e) => Err(e),
        }
    }

    pub fn send_welcome(&mut self, player_name: PlayerName, grid: Grid) -> ProtocolResult<()> {
        let read_timeout = self.conn.timeouts.read.clone();
        self.conn.send(WelcomeMessage {
            player_name: player_name,
            grid: grid,
            timeout: read_timeout,
        })
    }

    pub fn send_new_game(&mut self, game_state: GameState) -> ProtocolResult<()> {
        self.conn.send(NewGameMessage { game: game_state })
    }

    pub fn send_new_turn(&mut self, turn_state: TurnState) -> ProtocolResult<()> {
        self.conn.send(TurnMessage { turn: turn_state })
    }

    pub fn recieve_next_move(&mut self) -> ProtocolResult<Direction> {
        let move_: ProtocolResult<MoveMessage> = self.conn.recieve();
        match move_ {
            Ok(MoveMessage { direction }) => Ok(direction),
            Err(e) => Err(e),
        }
    }

    pub fn send_death(&mut self, cause_of_death: CauseOfDeath) -> ProtocolResult<()> {
        self.conn.send(DiedMessage { cause_of_death: cause_of_death })
    }

    pub fn send_won(&mut self) -> ProtocolResult<()> {
        self.conn.send(WonMessage {})
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerState {
    New,
    Version,
    Identify { desired_player_name: PlayerName },
    Ready,
    Playing,
    Turning,
    Moving { direction: Direction },
    Dead,
    Won,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    Versioning,
    Identifying,
    Welcoming { player_name: PlayerName, grid: Grid },
    NewGame { game: GameState },
    NewTurn { turn: TurnState },
    Move,
    Death { cause_of_death: CauseOfDeath },
    Victory,
    GameEnds,
}

impl PlayerState {
    pub fn next(self,
                connection: &mut PlayerConnection,
                event: PlayerEvent)
                -> ProtocolResult<PlayerState> {
        match (self, event) {
            // New players can be versioned.
            (PlayerState::New, PlayerEvent::Versioning) => {
                connection.send_version()?;
                Ok(PlayerState::Version)
            }
            // All versioned messages may send identity.
            (PlayerState::Version, PlayerEvent::Identifying) => {
                let desired_player_name = connection.recieve_identify()?;
                Ok(PlayerState::Identify { desired_player_name: desired_player_name })
            }
            // All identified players can be welcomed.
            (PlayerState::Identify { .. }, PlayerEvent::Welcoming { ref player_name, grid }) => {
                connection.send_welcome(player_name.clone(), grid)?;
                Ok(PlayerState::Ready)
            }
            // All non-playing players can begin new games.
            (PlayerState::Ready, PlayerEvent::NewGame { game }) => {
                connection.send_new_game(game)?;
                Ok(PlayerState::Playing)
            }
            // Playing or Dead players send turn messages.
            (PlayerState::Playing, PlayerEvent::NewTurn { turn }) => {
                connection.send_new_turn(turn)?;
                Ok(PlayerState::Turning)
            }
            (PlayerState::Dead, PlayerEvent::NewTurn { turn }) => {
                connection.send_new_turn(turn)?;
                Ok(PlayerState::Dead)
            }
            // Playing players recieve move messages. Dead players do not.
            (PlayerState::Turning, PlayerEvent::Move) => {
                let direction = connection.recieve_next_move()?;
                Ok(PlayerState::Moving { direction: direction })
            }
            // Living players die and send cause of death message.
            (PlayerState::Playing, PlayerEvent::Death { cause_of_death }) => {
                connection.send_death(cause_of_death)?;
                Ok(PlayerState::Dead)
            }
            // Living players win and send won message.
            (PlayerState::Playing, PlayerEvent::Victory) => {
                connection.send_won()?;
                Ok(PlayerState::Won)
            }
            // Won or dead players wait until game ends.
            (PlayerState::Won, PlayerEvent::GameEnds) => Ok(PlayerState::Ready),
            (PlayerState::Dead, PlayerEvent::GameEnds) => Ok(PlayerState::Ready),
            // Errored players die.
            (current_state, invalid_event) => {
                Err(ProtocolError::InvalidStateTransition {
                    from_state: Box::new(current_state),
                    event: invalid_event,
                })
            }
        }
    }
}

#[derive(Debug)]
pub struct PlayerAgent {
    pub state: ProtocolResult<PlayerState>,
    pub connection: PlayerConnection,
}

impl PlayerAgent {
    pub fn new(connection: PlayerConnection) -> PlayerAgent {
        PlayerAgent {
            state: Ok(PlayerState::New),
            connection: connection
        }
    }

    pub fn next(&mut self, event: PlayerEvent) -> Option<PlayerState> {
        if self.state.is_ok() {
            let state = self.state.as_mut().unwrap().clone();
            self.state = state.next(&mut self.connection, event);
        }
        match self.state.as_mut() {
            Ok(v) => Some(v.clone()),
            Err(_) => None
        }
    }
}
