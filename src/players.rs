pub struct Players {
    pub items: HashMap<PlayerName, Player>
}

impl Players {
    pub fn new() -> Players {
        Players { items: HashMap::new() }
    }

    pub fn add(&mut self, player_name: PlayerName, player_connection: PlayerConnection) {
        self.items.insert(player_name.clone(), Player::new(player_name, player_connection));
    }

    pub fn remove(&mut self, player_name: PlayerName) -> Option<Player> {
        self.items.remove(&player_name)
    }

    pub fn send(&mut self,
                player_name: PlayerName,
                command: Command)
                -> Result<(), ProtocolError> {
        self.i
            .get_mut(&player_name)
            .ok_or(ProtocolError::SendToUnknownPlayer)?
            .write(&command)
    }

    pub fn send_to_all(&mut self,
                     command: Command)
                     -> HashMap<PlayerName, StdResult<(), ProtocolError>> {
        let mut result_pairs = Vec::with_capacity(self.connections.len());
        self.connections
            .par_iter_mut()
            .map(|(player_name, connection)| (player_name.clone(), connection.write(&command)))
            .collect_into(&mut result_pairs);
        result_pairs.into_iter().collect()
    }

    pub fn recieve(&mut self, player_name: PlayerName) -> StdResult<Command, ProtocolError> {
        self.connections
            .get_mut(&player_name)
            .ok_or(ProtocolError::RecieveFromUnknownPlayer)?
            .read()
    }

    pub fn recieve_from_all(&mut self) -> HashMap<PlayerName, StdResult<Command, ProtocolError>> {
        let mut result_pairs = Vec::with_capacity(self.connections.len());
        self.connections
            .par_iter_mut()
            .map(|(player_name, connection)| (player_name.clone(), connection.read()))
            .collect_into(&mut result_pairs);
        result_pairs.into_iter().collect()
    }
}
