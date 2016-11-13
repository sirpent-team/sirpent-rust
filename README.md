# Sirpent (in Rust)

## Configuring the Grid Tiling
Sirpent can use any of the [Regular Tilings](https://en.wikipedia.org/wiki/Euclidean_tilings_by_convex_regular_polygons#Regular_Tilings) for its Grid. Rust's type system is a little too limited to choose at runctime, so it's a compile-time option.

### Hexagonal (default)

Run the Server:

``` sh
cargo run
```

Run a client:

``` sh
cargo run --example dummy_client
```

### Square

**Not yet implemented (`SquareGrid::random_cell`).**

Run the Server:

``` sh
cargo run --no-default-features --features square
```

Run a client:

``` sh
cargo run --example dummy_client --no-default-features --features square
```

### Triangle

**Not yet implemented (`TriangleGrid::random_cell`).**

Run the Server:

``` sh
cargo run --no-default-features --features triangle
```

Run a client:

``` sh
cargo run --example dummy_client --no-default-features --features triangle
```
