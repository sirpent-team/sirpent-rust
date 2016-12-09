pub struct PlayerConnection {
    pub state: PlayerState,
    pub conn: ProtocolConnection,
}

impl PlayerConnection {
    pub handshake(&mut self, grid: Grid) -> Result<Player, ProtocolError> {
        self.send(Command::version())?;
        self.send(Command::Welcome {grid: grid, timeout: self.conn.timeouts.read})?;
        match self.recieve() {
            Ok(Command::Identify { player }) => Ok(player),
            Ok(_) => Err(ProtocolError::WrongCommand),
            Err(e) => Err(e)
        }.and_then(|player| self.send(Command::Identified { player_name: player.name.clone() }))
    }

    pub tell_new_game(&mut self, game_state: GameState) -> Result<(), ProtocolError> {
        self.send(Command::NewGame { game: game_state })
    }

    pub tell_turn(&mut self, turn_state: TurnState) -> Result<(), ProtocolError> {
        self.send(Command::Turn { turn: turn_state })
    }

    pub ask_next_move(&mut self) -> Result<Direction, MoveError> {
        match self.recieve() {
            Ok(Command::Move { direction }) => Ok(direction),
            Ok(_) => Err(From::from(ProtocolError::WrongCommand)),
            Err(e) => Err(From::from(e)),
        }
    }

    pub tell_death(&mut self, cause_of_death: CauseOfDeath) -> Result<(), ProtocolError> {
        self.send(Command::Died { cause_of_death: cause_of_death })
    }

    pub tell_won(&mut self, cause_of_death: CauseOfDeath) -> Result<(), ProtocolError> {
        self.send(Command::Died { cause_of_death: cause_of_death })
    }
}
