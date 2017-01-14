TODOS
=====

* Lay out possible errors (on advisory basis) and possible causes of death.
* Specify resolution to "In that case those players despite dying also win."

Sirpent Protocol v0.3-incomplete
================================
By David Morris and [Michael Mokrysz](https://github.com/46bit).

General Considerations
----------------------

All communication takes the form of messages between client and
server, as plain text over a TCP socket.

In this specification, lines starting with `>` are used to indicate
messages from client to server; lines with no prefix indicate
responses.

Each message is a JSON object, sent on exactly one line of text as delimited by
`\n`. Newlines embedded in the JSON are are not permitted. Carriage returns
should be handled so as to allow `\r\n` delimiters.

All messages have the same top-level structure:

    {
        "msg": "<kind>",
        "data: {...}
    }

Other header information (keys in the top-level object) may be
added in a future protocol version. Participants must ignore unknown
keys in the header.

Values of `msg` are drawn from a fixed set (per protocol version).
`data` is optional, but if present its value must always be an object,
even when there is only one value; this is to ease backwards compatibility.
Participants must ignore unknown keys in `data`.

The purpose of these constraints is to ease implementation in the
largest number of languages and environments:

  - The JSON parser only needs to be able to do String -> JSON (which
    is usually the first example given for using a JSON libary).
  - Message dispatch is straightforward: `msg` values map to
    functions, all of which can be of the same type, with a single
    argument (`data`).

Player sessions are stateful and will experience a single game at a time, with
each TCP socket experiencing a sequence of games. Spectators will see the data
for all ongoing games on their TCP socket.


Board structure
---------------

All games are played on a grid which is a tiling of cells. Clients can implement
the grids, or ease implementation by retrieving a graph representation.

Clients are be told the current grid type during the initial handshake:

    Server: {"msg": "welcome", "data": {…, "grid": {"kind": "hexagon", "data": {radius": 15}}}}

If clients wish to be sent a graph representation they must ask now. It will be
sent as an adjacency relation.

    Client: {"msg": "describe_grid"}
    Server: {"msg": "grid_graph", "data": {"edges": [[{"x": 0, "y": 0}, {"x": 0, "y": 1}], [{"x": 0, "y": 1}, {"x": 1, "y": 1}], [{"x": 1, "y": 1},{"x": 1, "y": 0}], [{"x": 1, "y": 0}, {"x": 0, "y": 0}], [{"x": 0, "y": 1}, {"x": 0, "y": 0}], [{"x": 1, "y": 1}, {"x": 0, "y": 1}], [{"x": 1, "y": 0},"1,1"], [{"x": 0, "y": 0},{"x": 1, "y": 0}]]]]}}

The cells will take varying representations depending upon the underlying grid
type but all will be dictionaries from strings to 64-bit signed integers.

In general, clients should be able to navigate the graph without regard to the
spatial positioning of cells. But it is recommended clients implement the
underlying grid systems. A contextless graph has implications such as requiring
pathfinding for all movement planning.

Dynamic and Static State
------------------------

The board state is divided into dynamic and static state.

Static state is essentially the shape of the board (i.e., the graph). It is
guaranteed not to change during a TCP socket; no guarantees are made between
sockets.

    {"game": {"grid": {"radius": 25}, "players": ["46bit", "46bit_"], "uuid": "bb117ad4-d26b-49ac-8cd1-2d30572e6f41"}, "game_id": "bb117ad4-d26b-49ac-8cd1-2d30572e6f41"}

Dynamic state consists of the positions of the snakes and any food on the board:

    {"turn": {"casualties": {}, "eaten":{}, "food": [{"x": -24, "y": 3}], "snakes": {"46bit": {"segments": [{"x": -6, "y": -17}]}, "46bit_": {"segments": [{"x": 11,"y": -1}]}}, "turn_number": 0}, "game_id": "bb117ad4-d26b-49ac-8cd1-2d30572e6f41"}

`turn.casualties` should give a dictionary from player names to causes of death.
This must only consist of players who died in the previous turn. If clients wish
to track death information they should aggregate this information themselves.

`turn.eaten` should give a dictionary from player names to vectors where food
was consumed in the previous turn.

`turn.food` must be a list of cells containing food. `turn.snakes` must be a
dictionary from living player names to living snakes.

Each snake is specified as a dictionary with a list of cells, e.g.:

    {"segments": [{"x": 0, "y": 0}, {"x": 0, "y": 1}]}

The first element of the segments list is the head of the snake.

Subsequent extensions may introduce additional kinds of dynamic state.

Gameplay
--------

?? Food ??

?? Growth ??

?? Fun ??

?? Profit ??

Session structure
-----------------

### Handshake

#### Version negotiation

Sessions start with version negotation. The server tells the client which
protocol version and sirpent version it is running. Clients may close the socket
if they don't support that or else attempt to continue.

    Server: {"msg": "version", "data": {"protocol": "0.3", "sirpent": "0.2.0"}}

#### Name registration

The client then registers with the server, offering a preferred name:

    Client: {"msg": "register", "data": {"desired_name": "46bit", "kind": "player"}}

Names cannot contain a literal `\n` but may be arbitrary valid unicode.

#### Welcome and naming

The server replies with a welcome message.

    Server: {"msg": "welcome", "data": {"grid": {"radius": 25}, "name": "46bit", "timeout": {"nanos": 0,"secs": 5}}}

It may offer a different name to that offered by the client; the client must
then use this name (for example, the server might add a suffix to distinguish
multiple connections from the same client).

As discussed it will inform of a particular grid being used. Clients may be
timed out if an response takes longer than allowed and those details should be
communicated here.

If a client does not want to continue it should close the socket.

#### Optionally requesting grid adjacency matrix

As discussed above the client can ask for an adjacency matrix of the grid being
used. Unless the client needs this it should proceed to the next stage.

    Client: {"msg": "describe_grid"}
    Server: {"msg": "grid_graph", "data": {"edges": [[{"x": 0, "y": 0}, {"x": 0, "y": 1}], [{"x": 0, "y": 1}, {"x": 1, "y": 1}], [{"x": 1, "y": 1},{"x": 1, "y": 0}], [{"x": 1, "y": 0}, {"x": 0, "y": 0}], [{"x": 0, "y": 1}, {"x": 0, "y": 0}], [{"x": 1, "y": 1}, {"x": 0, "y": 1}], [{"x": 1, "y": 0},"1,1"], [{"x": 0, "y": 0},{"x": 1, "y": 0}]]]]}}

#### Finishing the handshake

The client then sends a `ready` message to indicate they are ready to join a
game.

    Client: {"msg": "ready"}

### Playing

#### Starting a new game

The server will decide when to play a new game. It can choose its own criteria
although generally this would be based upon having a sensible number of players
connected.

Once a new game is started, the server will send a `game_start` message
describing the static state of the game. This will be sent to all participating
clients, both players and spectators:

    Server: {"msg": "game_start", "data": {"game": {"grid": {"radius": 25}, "players": ["46bit", "46bit_"], "id": "bb117ad4-d26b-49ac-8cd1-2d30572e6f41"}, "game_id": "bb117ad4-d26b-49ac-8cd1-2d30572e6f41"}}

Each game has a UUID. In `game_start` this is awkwardly included twice to be
consistent with where other within-game messages expect it in `data.game_id`.

#### Performing turns

Immediately after `game_start` or moves being made, the server will send a
`turn` message. This contains the current dynamic state of the game. It will
be sent to all participating clients, both players and spectators:

    Server: {"msg": "turn", "data": {"turn": {"casualties": {"46bit_": …}, "eaten":{}, "food": [{"x": -24, "y": 3}], "snakes": {"46bit": {"segments": [{"x": -6, "y": -17}]}, "46bit_": {"segments": [{"x": 11,"y": -1}]}}, "turn_number": 0}, "game_id": "bb117ad4-d26b-49ac-8cd1-2d30572e6f41"}}

Turn numbers must be zero-indexed and incremented each turn.

If clients wish to track death and eating information they should aggregate this
information themselves.

Living players must then send a `move` message indicating the direction in
which they wish to move the head of their snake:

    Client: {"msg": "move", "data": {"direction": "north"}}

If `describe_grid` is implemented a player must be able to specify the cell the
head should move to instead. This is useful if working with a graph
representation:

    Client: {"msg": "move", "data": {"next": {"x": 5, "y": 6}}}

Clients must only send one of these representations. They must be valid.

#### Notifications of death

The server will compute the resulting turn. It will determine which snakes have
collided with each other, with the edge of the grid, or have errored in some
fashion (e.g., no move received inside the timeout).

Before the next `turn` message each newly dead player will receive a `died`
message giving their cause of death:

    Server: {"msg": "died", "data": {"cause_of_death": …, "game_id": "bb117ad4-d26b-49ac-8cd1-2d30572e6f41"}}

The receipt of a `died` message should not result in a terminated TCP socket and
should not stop information on the ongoing game. Dead players should be provided
with the game action similarly to spectators. They should continue to receive
`turn` messages but must not send `move` messages.

If a client does not support this then it should close the TCP socket upon death
and immediately reconnect to wait for the next game. Servers should have a pause
between games sufficient to allow reconnections to happen.

#### Notifications of victory and the game ending

The server chooses victory criteria. Generally this is when only one player is
left standing or when all players die. The latter case could happen when N
remaining snakes die in the same turn. In that case those players (despite
dying) also win.

Winning players will be sent a `won` message:

    Server: {"msg": "won", "data": {"game_id": "bb117ad4-d26b-49ac-8cd1-2d30572e6f41"}}

All participating clients, both players and spectators, will then receive a
`game_over` message. This also contains the final state.

    Server: {"msg": "game_over", "data": {"winners": ["46bit", "Taneb"], "turn": {"casualties": {"Taneb": …}, "eaten": {}, "food": [{"x": -24, "y": 3}], "snakes": {"46bit": {"segments": [{"x": -6, "y": -17}]}, "46bit_": {"segments": [{"x": 11,"y": -1}]}}, "turn_number": 100}, "game_id": "bb117ad4-d26b-49ac-8cd1-2d30572e6f41"}}

### Spectating

Clients can also register as spectators. The expected use case for this is
scoreboards and visualizers. Spectators must send no messages after the initial
handshake.

    Server: {"msg": "version", "data": {"protocol": "0.3", "sirpent": "0.2.0"}}
    Client: {"msg": "register", "data": {"desired_name": "visualiser", "kind": "spectator"}}
    Server: {"msg": "welcome", "data": {"grid": {"radius": 25}, "name": "spectator", "timeout": {"nanos": 0,"secs": 5}}}
    Client: {"msg": "ready"}

    Server: {"msg": "game_start", "data": {"game": {"grid": {"radius": 25}, "players": ["46bit", "46bit_"], "uuid": "bb117ad4-d26b-49ac-8cd1-2d30572e6f41"}, "game_id": "bb117ad4-d26b-49ac-8cd1-2d30572e6f41"}}
    Server: {"msg": "turn", "data": {"turn": {"casualties": {}, "eaten":{}, "food": [{"x": -24, "y": 3}], "snakes": {"46bit": {"segments": [{"x": -6, "y": -17}]}, "46bit_": {"segments": [{"x": 11,"y": -1}]}}, "turn_number": 0}, "game_id": "bb117ad4-d26b-49ac-8cd1-2d30572e6f41"}}
    […]
    Server: {"msg": "game_over", "data": {"winners": ["46bit", "Taneb"], "turn": {"casualties": {"Taneb": …}, "eaten": {}, "food": [{"x": -24, "y": 3}], "snakes": {"46bit": {"segments": [{"x": -6, "y": -17}]}, "46bit_": {"segments": [{"x": 11,"y": -1}]}}, "turn_number": 100}, "game_id": "bb117ad4-d26b-49ac-8cd1-2d30572e6f41"}}

Died and won messages must not be relayed to spectators. Spectators should infer
deaths from the `data.turn.casualties` field on each `turn` or `game_over`
message. Similarly winners should be inferred from the `data.winners` field on
each `game_over` message.

Servers may play multiple games simultaneously. Spectators may be sent multiple
games at once. In this case the `data.game_id` field can be used to determine
which game a message refers to.

Errors
------

If a client sends an incorrect move (i.e., to a cell non-adjacent to
the head, or overlapping the tail), the server responds with
`move_error`:

    > {{"msg": "move", "data": {"direction": "atotallyinvaliddirection"}}
    {"resp": "move_error", "data": {"error_msg": "Invalid move"}}

If a client sends a message which is invalid in the current session
state (e.g., sending a move after a game has finished) the server
responds with `state_error`:

    > {"msg": "move", "data": {"next": "0,0"}}
    {"resp": "state_error", "data": {"error_msg": "Game over"}}

If there is any other kind of error, the server responds with a
generic `error`:

    > {"msg": "flibbertigibbet"}
    {"resp": "error", "data": {"error_msg": "wat"}}

For all errors the `data` key of the response is optional and clients must not
depend on it. The server may include additional helpful information in `data`,
if it is feeling magnanimous.


Timeouts
--------

Servers may give clients a fixed time to send messages. If a client fails to
respond quick enough the connection may be closed. If closing the connection for
this an error message indicating such should be sent.

In the case of player moves timing out the server should handle this without
terminating the client, killing them for the missing move but retaining their
connection as for any other dead player.

Game IDs
--------

All messages generated by the server for a given game must have the same game
id stored in `data.game_id`. This is not important to Player clients
(because they only handle one game at a time) but is intended to allow
Spectator clients to demux games straightforwardly.

Game ids must be as unique as reasonably possible (any guid algorithm should be
sufficient). Servers may use a random (V4) UUID as a sensible approach.

Protocol States
---------------

A player session can be in one of the following states (valid messages
listed):

- `PRE_VERSION`
  + `version`
- `PRE_REGISTER`
  + `register`
- `PRE_WELCOME`
  + `welcome`
- `PRE_READY`
  + `describe_grid`
  + `ready`
- `READY_WAIT`
  + `game_start`
- `GAME_PLAYING`
  + `turn`
- `TURN_MOVING`
  + `move`
- `TURN_DONE`
  + `died`
  + `won`
  + `game_over`

A spectator session can be in one of the following states (valid messages listed):

- `PRE_VERSION`
  + `version`
- `PRE_REGISTER`
  + `register`
- `PRE_WELCOME`
  + `welcome`
- `PRE_READY`
  + `describe_grid`
  + `ready`
- `READY_WAIT`
  + `game_start`
- `GAME_PLAYING`
  + `turn`
  + `game_over`
