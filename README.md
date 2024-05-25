# relay

relay is a token storage for [steel](https://github.com/TicClick/steel) which refreshes osu! API tokens in the background, so that users don't have to re-acquire them every day.

## run locally

- install [Rust](https://www.rust-lang.org/) and [Docker Compose](https://docs.docker.com/compose/install/)
- [register](https://osu.ppy.sh/home/account/edit#oauth) your copy of relay
- copy [`config.template.yaml`](./config.template.yaml) to `config.yaml` and fill it in as necessary

```sh
docker compose up --build && RUST_LOG=info cargo run --release
```
