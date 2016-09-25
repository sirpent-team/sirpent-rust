use uuid::Uuid;

#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    pub snake_uuid: Uuid,
}

impl Player {
    pub fn new(name: String, secret: String, snake_uuid: Uuid) -> Player {
        Player {
            name: name,
            snake_uuid: snake_uuid,
        }
    }
}
