```
(❀◠‿◠) ~poem~2~poem~> (◕‿◕✿)
```
# poem2poem

This is a toy program to familiarize myself with the [iroh](https://iroh.rs/) p2p library.

On startup the program asks to choose a username. It then takes a poem as input (max 100 character text), which it writes, along with the username and timestamp, as a blob and replicates to the p2p network.

#### Run

```bash
cargo run
```

#### Run with nix

```bash
nix run
```
