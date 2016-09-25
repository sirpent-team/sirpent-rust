use uuid::Uuid;

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    #[serde(skip_serializing)]
    pub secret: Option<String>,
    // @TODO: Semantically GameState should have a field to map Player->Snake, as it isn't
    // part of a Player except during a game.
    pub snake_uuid: Option<Uuid>,
}

impl Player {
    pub fn new(name: String, secret: Option<String>, snake_uuid: Uuid) -> Player {
        Player {
            name: name,
            secret: secret,
            snake_uuid: Some(snake_uuid),
        }
    }
}
