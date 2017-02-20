```
# ======== SERVER COMMS MODEL ========= #
#     ↑↓      PLAYER         SPECTATOR  #
# 01  T       version        version    #
# 02   R      register       register   #
# 03  T       welcome        welcome    #
# -------------  [...]  --------------- #
# 04  T       game           game       #
# 05  T       round          round      #
# 06   R      move           move       #
# --------  JMP 05 if ongoing  -------- #
# 07  T       outcome        outcome    #
# -------------  JMP 04  -------------- #
# ===================================== #

# ======== SERVER COMMS MODEL ========= #
#     ↑↓      PLAYER         SPECTATOR  #
# 01  R       version        version    #
# 02   T      register       register   #
# 03  R       welcome        welcome    #
# -------------  [...]  --------------- #
# 04  R       game           game       #
# 05  R       round          round      #
# 06   T      move           move       #
# -----------  CAN JMP 05  ------------ #
# 07  R       outcome        outcome    #
# -------------  JMP 04  -------------- #
# ===================================== #
```

{"register": {"desired_name": "your_players_name", "kind": "player"}}
{"register": {"desired_name": "your_players_name", "kind": "spectator"}}
{"move": {"direction": "north"}}


``` json
{"msg": "version", "sirpent": "X.X.X", "protocol": "0.4"}
{"msg": "register", "desired_name": "your_players_name", "kind": "player"}
{"msg": "register", "desired_name": "your_players_name", "kind": "spectator"}
{"msg": "welcome", "name": "your_players_name_", "grid": _, "timeout_millis": 5000}
{"msg": "game", "game": _}
{"msg": "round", "round": _, "game_uuid": "123e4567-e89b-12d3-a456-426655440000"}
{"msg": "move", "direction": "north"}
{"msg": "outcome", "winners": ["player1"], "conclusion": _, "game_uuid": "123e4567-e89b-12d3-a456-426655440000"}
```

``` json
GRID CONFIG: `welcome.grid` and `game.game.grid`
{
  "tiling": "hexagon",
  "radius": 25
}

GAME STATE: `game.game`
{
  "uuid": "2e44d843-a320-41ae-b00d-c524275c1590",
  "grid": {
    "tiling": "hexagon",
    "radius": 25
  },
  "players": [
    "your_players_name__",
    "your_players_name___"
  ]
}

ROUND STATE: `round.round` and `outcome.conclusion`
{
  "round_number": 0,
  "food": [{"x": -11, "y": 2}],
  "eaten": {},
  "snakes": {
    "living_player_1": {
      "segments": [{"x": 7, "y": 3}]
    },
    "living_player_2": {
      "segments": [{"x": -10, "y": 16}]
    }
  },
  "directions": {
    "living_player_1": "north",
    "living_player_2": "northeast",
    "dead_player_1": "southwest"
  },
  "casualties": {
    "dead_player_1": "no_move_made",
    "dead_player_2": "collided_with_snake",
    "dead_player_3": "collided_with_bounds"
  }
}
```
